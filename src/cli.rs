extern crate clap;
use clap::{Arg, App, SubCommand};
use kube::CustomResourceExt;

fn main() {
    let matches = App::new("arcanum-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .subcommand(SubCommand::with_name("gencrd")
            .about("generate CRDs arcanum needs to function")
        )
        .subcommand(SubCommand::with_name("genkey")
            .about("generate a keypair for arcanum")
        )
        .subcommand(SubCommand::with_name("encrypt")
            .about("encrypt a string")
            .arg(Arg::with_name("STRING")
                .help("sets the string to encrypt")
                .required(true))
        )
        .get_matches();

    let mut csprng = rand_core::OsRng;

    if let Some(matches) = matches.subcommand_matches("gencrd") {
        print!("{}", serde_yaml::to_string(&arcanum::SyncedSecret::crd()).unwrap())
    }

    if let Some(matches) = matches.subcommand_matches("genkey") {
        let (secret, public) = ecies_ed25519::generate_keypair(&mut csprng);
        println!("export ARCANUM_PUB_KEY={}", base64::encode(public.as_bytes()));
        println!("export ARCANUM_ENC_KEY={}", base64::encode(secret.as_bytes()));
    }

    if let Some(matches) = matches.subcommand_matches("encrypt") {
        let secret = ecies_ed25519::SecretKey::from_bytes(
            &*base64::decode(std::env::var("ARCANUM_ENC_KEY").unwrap()).unwrap(),
        )
            .unwrap();
        let public = ecies_ed25519::PublicKey::from_secret(&secret);
        println!(
            "{}",
            base64::encode(ecies_ed25519::encrypt(&public, matches.value_of("STRING").unwrap().as_bytes(), &mut csprng).unwrap())
        );
    }
}
