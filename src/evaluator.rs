//! Task evaluator for running agents against tasks

use crate::docker::{ContainerRun, DockerConfig, DockerExecutor};
use crate::llm_client::Agent;
use crate::task::{Task, TaskResult};
use crate::terminal_harness::{HarnessConfig, TerminalHarness};
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
                if write_result.is_err() {
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

        // Run the agent using terminal harness
        info!("Running agent on task with terminal harness...");
        let harness_result = self
            .run_agent_in_container(
                &container,
                agent,
                task.instruction(),
                task.config.timeout_secs as u64,
                50, // max_steps
            )
            .await;

        // Get agent output from harness
        let agent_output = match &harness_result {
            Ok(result) => {
                let mut output = String::new();
                for step in &result.steps {
                    output.push_str(&format!(
                        "=== Step {} ===\nCommand: {:?}\nExit: {}\nOutput:\n{}\n\n",
                        step.step,
                        step.command,
                        step.exit_code,
                        step.output
                    ));
                }
                output
            }
            Err(e) => format!("Harness error: {}", e),
        };

        // Log harness result
        match &harness_result {
            Ok(result) => {
                info!(
                    "Harness completed: steps={}, task_complete={}",
                    result.steps.len(),
                    result.task_complete
                );
            }
            Err(e) => {
                warn!("Harness failed: {}", e);
            }
        }

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

    /// Run the agent inside the container using terminal harness
    async fn run_agent_in_container(
        &self,
        container: &ContainerRun,
        agent: &AgentInfo,
        instruction: &str,
        task_timeout_secs: u64,
        max_steps: u32,
    ) -> Result<crate::terminal_harness::HarnessResult> {
        info!(
            "Running agent {} (max_steps={}, timeout={}s)",
            agent.hash, max_steps, task_timeout_secs
        );

        // Configure harness
        let harness_config = HarnessConfig {
            max_steps,
            step_timeout_secs: 60,
            total_timeout_secs: task_timeout_secs,
            working_dir: "/app".to_string(),
        };

        // Create terminal harness
        let mut harness = TerminalHarness::new(container, harness_config);

        // Agent must provide source code - this is a code challenge
        let code = match &agent.source_code {
            Some(code) if !code.trim().is_empty() => code.clone(),
            _ => {
                return Err(anyhow::anyhow!("No agent source code provided - submission rejected"));
            }
        };

        info!("Running agent code ({} bytes)", code.len());
        harness
            .run(instruction, |request| {
                let code = code.clone();
                async move {
                    let agent = Agent::from_source(code);
                    agent.execute(request).await
                }
            })
            .await
    }

    /// Evaluate an agent on multiple tasks
    pub async fn evaluate_tasks(&self, tasks: &[&Task], agent: &AgentInfo) -> Vec<TaskResult> {
        self.evaluate_tasks_with_progress(tasks, agent, None::<fn(u32, u32, &TaskResult)>)
            .await
    }

    /// Evaluate an agent on multiple tasks with progress callback
    /// The callback is called after each task completes with (task_index, total_tasks, result)
    pub async fn evaluate_tasks_with_progress<F>(
        &self,
        tasks: &[&Task],
        agent: &AgentInfo,
        progress_callback: Option<F>,
    ) -> Vec<TaskResult>
    where
        F: Fn(u32, u32, &TaskResult) + Send + Sync,
    {
        let mut results = Vec::new();
        let total_tasks = tasks.len() as u32;

        for (index, task) in tasks.iter().enumerate() {
            let task_index = (index + 1) as u32;

            let result = match self.evaluate_task(task, agent).await {
                Ok(result) => result,
                Err(e) => {
                    error!("Evaluation error for task {}: {}", task.id(), e);
                    TaskResult::failure(
                        task.id().to_string(),
                        agent.hash.clone(),
                        0,
                        String::new(),
                        String::new(),
                        format!("Evaluation error: {}", e),
                    )
                }
            };

            // Call progress callback if provided
            if let Some(ref callback) = progress_callback {
                callback(task_index, total_tasks, &result);
            }

            info!(
                "Task [{}/{}] completed: {} - passed={} score={:.2}",
                task_index,
                total_tasks,
                task.id(),
                result.passed,
                result.score
            );

            results.push(result);
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
