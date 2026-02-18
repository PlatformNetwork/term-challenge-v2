use sp_core::{sr25519, Pair};

fn main() {
    let secret_hex = "0000000000000000000000000000000000000000000000000000000000000001";
    let bytes = hex::decode(secret_hex).expect("Invalid hex");
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);

    // Use sr25519 (Substrate/Bittensor standard)
    let pair = sr25519::Pair::from_seed(&arr);
    let public = pair.public();
    println!("{}", hex::encode(public.0));
}
