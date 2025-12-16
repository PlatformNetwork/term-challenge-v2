//! Task evaluator for running agents against tasks

use crate::docker::{ContainerRun, DockerConfig, DockerExecutor};
use crate::task::{Task, TaskResult};
use anyhow::Result;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Agent information
#[derive(Clone, Debug)]
pub struct AgentInfo {
    /// Agent hash (unique identifier)
    pub hash: String,
    /// Agent Docker image
    pub image: String,
    /// Agent API endpoint (if applicable)
    pub endpoint: Option<String>,
    /// Source code (if not using pre-built image)
    pub source_code: Option<String>,
}

/// Task evaluator
pub struct TaskEvaluator {
    docker: DockerExecutor,
    /// Maximum concurrent evaluations (reserved for future parallel evaluation)
    #[allow(dead_code)]
    max_concurrent: usize,
}

impl TaskEvaluator {
    /// Create a new evaluator
    pub async fn new(max_concurrent: usize) -> Result<Self> {
        let docker = DockerExecutor::new().await?;
        Ok(Self {
            docker,
            max_concurrent,
        })
    }

    /// Evaluate an agent on a single task
    pub async fn evaluate_task(&self, task: &Task, agent: &AgentInfo) -> Result<TaskResult> {
        info!("Evaluating agent {} on task {}", agent.hash, task.id());

        let start = Instant::now();

        // Create Docker config from task config
        let docker_config = DockerConfig {
            memory_limit: task.config.memory_limit.clone(),
            cpu_limit: task.config.cpu_limit,
            timeout_secs: task.config.timeout_secs as u64,
            network_mode: task.config.network_mode.clone(),
            env: {
                let mut env = task.config.env.clone();
                env.push("TEST_DIR=/tests".to_string());
                env
            },
            working_dir: "/app".to_string(),
        };

        // Determine image to use
        // If agent has source code, we use the task's docker image (environment)
        // and inject the code. Otherwise we use the agent's image.
        let image_to_run = if agent.source_code.is_some() {
            &task.config.docker_image
        } else {
            &agent.image
        };

        // Run the agent container
        let container = match self
            .docker
            .run_agent(
                &task.config.docker_image, // Ensure this base image exists
                image_to_run,
                task.path.as_deref(),
                &docker_config,
            )
            .await
        {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to create container: {}", e);
                return Ok(TaskResult::failure(
                    task.id().to_string(),
                    agent.hash.clone(),
                    start.elapsed().as_millis() as u64,
                    String::new(),
                    String::new(),
                    format!("Failed to create container: {}", e),
                ));
            }
        };

        // Start the container
        if let Err(e) = container.start().await {
            container.remove().await.ok();
            return Ok(TaskResult::failure(
                task.id().to_string(),
                agent.hash.clone(),
                start.elapsed().as_millis() as u64,
                String::new(),
                String::new(),
                format!("Failed to start container: {}", e),
            ));
        }

        // Run setup script if present
        if let Some(setup_script) = &task.setup_script {
            debug!("Running setup script");
            let setup_result = container.exec(&["sh", "-c", setup_script]).await;
            if let Err(e) = setup_result {
                warn!("Setup script failed: {}", e);
            }
        }

        // Copy test files to container
        if !task.test_files.is_empty() {
            debug!("Copying test files to /tests");
            let mkdir_result = container.exec(&["mkdir", "-p", "/tests"]).await;
            if let Err(e) = mkdir_result {
                warn!("Failed to create /tests directory: {}", e);
            }

            for (filename, content) in &task.test_files {
                let file_path = format!("/tests/{}", filename);
                let write_result = container
                    .exec(&[
                        "sh",
                        "-c",
                        &format!(
                            "cat <<EOF > {}\n{}\nEOF",
                            file_path,
                            content.replace("$", "\\$").replace("`", "\\`") // Basic escape
                        ),
                    ])
                    .await;

                // Fallback to simpler echo if cat heredoc fails or complex content
                if let Err(_) = write_result {
                    let _ = container
                        .exec(&[
                            "sh",
                            "-c",
                            &format!("echo '{}' > {}", content.replace("'", "'\\''"), file_path),
                        ])
                        .await;
                }
            }
        }

        // Provide the task instruction to the agent
        let instruction_result = container
            .exec(&[
                "sh",
                "-c",
                &format!(
                    "echo '{}' > /app/INSTRUCTION.txt",
                    task.instruction().replace("'", "'\\''")
                ),
            ])
            .await;

        if let Err(e) = instruction_result {
            warn!("Failed to write instruction: {}", e);
        }

        // Inject source code if present
        if let Some(code) = &agent.source_code {
            debug!("Injecting agent source code");
            let inject_result = container
                .exec(&[
                    "sh",
                    "-c",
                    &format!(
                        "mkdir -p /agent && echo '{}' > /agent/main.py",
                        code.replace("'", "'\\''")
                    ),
                ])
                .await;

            if let Err(e) = inject_result {
                warn!("Failed to inject source code: {}", e);
            }
        }

        // Run the agent (this is where the agent does its work)
        // The agent should read /app/INSTRUCTION.txt and perform the task
        info!("Running agent on task...");
        let _agent_result = self.run_agent_in_container(&container, agent).await;

        // Get agent output
        let agent_output = container.logs().await.unwrap_or_default();

        // Run the test script
        info!("Running test script");
        let test_result = container.run_test(&task.test_script).await;

        // Cleanup
        container.stop().await.ok();
        container.remove().await.ok();

        let execution_time_ms = start.elapsed().as_millis() as u64;

        match test_result {
            Ok(result) => {
                let test_output = result.output();
                if result.success() {
                    info!("Task {} PASSED for agent {}", task.id(), agent.hash);
                    Ok(TaskResult::success(
                        task.id().to_string(),
                        agent.hash.clone(),
                        execution_time_ms,
                        test_output,
                        agent_output,
                    ))
                } else {
                    info!(
                        "Task {} FAILED for agent {} (exit code {})",
                        task.id(),
                        agent.hash,
                        result.exit_code
                    );
                    Ok(TaskResult::failure(
                        task.id().to_string(),
                        agent.hash.clone(),
                        execution_time_ms,
                        test_output,
                        agent_output,
                        format!("Test failed with exit code {}", result.exit_code),
                    ))
                }
            }
            Err(e) => {
                error!("Test execution error: {}", e);
                Ok(TaskResult::failure(
                    task.id().to_string(),
                    agent.hash.clone(),
                    execution_time_ms,
                    String::new(),
                    agent_output,
                    format!("Test execution error: {}", e),
                ))
            }
        }
    }

    /// Run the agent inside the container
    async fn run_agent_in_container(
        &self,
        container: &ContainerRun,
        agent: &AgentInfo,
    ) -> Result<()> {
        // Check if agent has an endpoint (API-based agent)
        if let Some(endpoint) = &agent.endpoint {
            // For API-based agents, we'd communicate via HTTP
            // This is a simplified version
            debug!("Agent endpoint: {}", endpoint);
        }

        // For Docker-based agents, we exec into the container
        let cmd = if agent.source_code.is_some() {
            // Run injected python script
            vec!["python3", "/agent/main.py"]
        } else {
            // Run standard entrypoint
            vec![
                "sh", "-c",
                "if [ -f /agent/run.sh ]; then /agent/run.sh; elif [ -f /run.sh ]; then /run.sh; else echo 'No agent entrypoint found'; fi"
            ]
        };

        let result = container.exec(&cmd).await?;

        debug!("Agent execution result: exit_code={}", result.exit_code);
        Ok(())
    }

    /// Evaluate an agent on multiple tasks
    pub async fn evaluate_tasks(&self, tasks: &[&Task], agent: &AgentInfo) -> Vec<TaskResult> {
        let mut results = Vec::new();

        for task in tasks {
            match self.evaluate_task(task, agent).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Evaluation error for task {}: {}", task.id(), e);
                    results.push(TaskResult::failure(
                        task.id().to_string(),
                        agent.hash.clone(),
                        0,
                        String::new(),
                        String::new(),
                        format!("Evaluation error: {}", e),
                    ));
                }
            }
        }

        results
    }

    /// Evaluate an agent on all tasks in a registry
    pub async fn evaluate_all(
        &self,
        registry: &crate::task::TaskRegistry,
        agent: &AgentInfo,
    ) -> Vec<TaskResult> {
        let tasks: Vec<&Task> = registry.tasks().collect();
        self.evaluate_tasks(&tasks, agent).await
    }
}

/// Builder for configuring evaluations
pub struct EvaluationBuilder {
    tasks: Vec<String>,
    num_tasks: Option<usize>,
    difficulty: Option<crate::task::Difficulty>,
    timeout_override: Option<u64>,
}

impl EvaluationBuilder {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            num_tasks: None,
            difficulty: None,
            timeout_override: None,
        }
    }

    /// Add specific task IDs to evaluate
    pub fn with_tasks(mut self, task_ids: Vec<String>) -> Self {
        self.tasks = task_ids;
        self
    }

    /// Limit number of random tasks
    pub fn with_num_tasks(mut self, n: usize) -> Self {
        self.num_tasks = Some(n);
        self
    }

    /// Filter by difficulty
    pub fn with_difficulty(mut self, difficulty: crate::task::Difficulty) -> Self {
        self.difficulty = Some(difficulty);
        self
    }

    /// Override task timeouts
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_override = Some(timeout_secs);
        self
    }

    /// Get tasks to evaluate from registry
    pub fn get_tasks<'a>(&self, registry: &'a crate::task::TaskRegistry) -> Vec<&'a Task> {
        if !self.tasks.is_empty() {
            // Specific tasks requested
            self.tasks
                .iter()
                .filter_map(|id| registry.get(id))
                .collect()
        } else if let Some(difficulty) = self.difficulty {
            // Filter by difficulty
            let mut tasks = registry.tasks_by_difficulty(difficulty);
            if let Some(n) = self.num_tasks {
                tasks.truncate(n);
            }
            tasks
        } else if let Some(n) = self.num_tasks {
            // Random subset
            registry.random_tasks(n)
        } else {
            // All tasks
            registry.tasks().collect()
        }
    }
}

impl Default for EvaluationBuilder {
    fn default() -> Self {
        Self::new()
    }
}
