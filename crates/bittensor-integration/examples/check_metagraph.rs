use bittensor_rs::metagraph::sync_metagraph;
use bittensor_rs::BittensorClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to Bittensor finney...");

    let client = BittensorClient::new("wss://entrypoint-finney.opentensor.ai:443").await?;

    println!("Querying subnet 100 metagraph...\n");

    let metagraph = sync_metagraph(&client, 100).await?;

    println!("Total neurons: {}", metagraph.n);
    println!("\nTop 30 by effective stake (alpha + root):");
    println!("{:-<90}", "");

    // Collect stakes
    let mut stakes: Vec<(u16, String, u128, u128, u128)> = metagraph
        .neurons
        .iter()
        .map(|(uid, neuron)| {
            let alpha = neuron.stake;
            let root = neuron.root_stake;
            let total = alpha.saturating_add(root);
            let hotkey_str = format!("{}", neuron.hotkey);
            (*uid as u16, hotkey_str, alpha, root, total)
        })
        .filter(|(_, _, _, _, total)| *total > 0)
        .collect();

    // Sort by total stake descending
    stakes.sort_by(|a, b| b.4.cmp(&a.4));

    println!(
        "{:<6} {:<50} {:>12} {:>12} {:>12}",
        "UID", "Hotkey", "Alpha", "Root", "Total TAO"
    );
    println!("{:-<90}", "");

    for (uid, hotkey, alpha, root, total) in stakes.iter().take(30) {
        let alpha_tao = *alpha as f64 / 1_000_000_000.0;
        let root_tao = *root as f64 / 1_000_000_000.0;
        let total_tao = *total as f64 / 1_000_000_000.0;
        println!(
            "{:<6} {:<50} {:>12.2} {:>12.2} {:>12.2}",
            uid, hotkey, alpha_tao, root_tao, total_tao
        );
    }

    let gte_1000 = stakes
        .iter()
        .filter(|(_, _, _, _, t)| *t as f64 / 1e9 >= 1000.0)
        .count();
    let gte_100 = stakes
        .iter()
        .filter(|(_, _, _, _, t)| *t as f64 / 1e9 >= 100.0)
        .count();
    let gte_10 = stakes
        .iter()
        .filter(|(_, _, _, _, t)| *t as f64 / 1e9 >= 10.0)
        .count();
    let gt_0 = stakes.len();

    println!("\n{:-<90}", "");
    println!("Validators with >= 1000 TAO: {}", gte_1000);
    println!("Validators with >= 100 TAO:  {}", gte_100);
    println!("Validators with >= 10 TAO:   {}", gte_10);
    println!("Validators with > 0 TAO:     {}", gt_0);

    Ok(())
}
