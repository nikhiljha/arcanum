use kube::CustomResourceExt;
fn main() {
    print!(
        "{}",
        serde_yaml::to_string(&arcanum::SyncedSecret::crd()).unwrap()
    )
}
