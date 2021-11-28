fn main() {
    let mut csprng = rand_core::OsRng;
    // let (secret, public) = ecies_ed25519::generate_keypair(&mut csprng);
    // println!("{}", base64::encode(public.as_bytes()));
    // println!("{}", base64::encode(secret.as_bytes()));
    let secret = ecies_ed25519::SecretKey::from_bytes(
        &*base64::decode(std::env::var("ARCANUM_ENC_KEY").unwrap()).unwrap()
    ).unwrap();
    let public = ecies_ed25519::PublicKey::from_secret(&secret);
    println!("{}", base64::encode(ecies_ed25519::encrypt(&public, "hello".as_bytes(), &mut csprng).unwrap()));
}
