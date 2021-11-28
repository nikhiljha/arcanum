use crate::Error::ArcanumError;
use crate::{telemetry, AppConfig, Error, Result};
use chrono::prelude::*;
use ecies_ed25519::SecretKey;
use futures::{future::BoxFuture, FutureExt, StreamExt};
use k8s_openapi::api::core::v1::{ObjectReference, Secret};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use k8s_openapi::ByteString;
use kube::api::PostParams;
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
    client::Client,
    runtime::{
        controller::{Context, Controller, ReconcilerAction},
        events::{Event, EventType, Recorder, Reporter},
    },
    CustomResource, Resource,
};
use prometheus::{
    default_registry, proto::MetricFamily, register_histogram_vec, register_int_counter, HistogramOpts,
    HistogramVec, IntCounter,
};
use rand::Rng;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::hash_map::IntoIter;
use std::collections::BTreeMap;
use std::iter::FromIterator;
use std::iter::Map;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::RwLock,
    time::{Duration, Instant},
};
use tracing::{debug, error, event, field, info, instrument, trace, warn, Level, Span};

/// Our Foo custom resource spec
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(kind = "SyncedSecret", group = "njha.dev", version = "v1", namespaced)]
#[kube(status = "SyncedSecretStatus")]
pub struct SyncedSecretSpec {
    data: HashMap<String, String>,
    pub_key: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct SyncedSecretStatus {
    reconciled: Option<bool>,
    last_updated: Option<DateTime<Utc>>,
}

// Context for our reconciler
#[derive(Clone)]
struct Data {
    /// kubernetes client
    client: Client,
    /// In memory state
    state: Arc<RwLock<State>>,
    /// Various prometheus metrics
    metrics: Metrics,
    /// App configuration
    config: AppConfig,
}

fn get_from_vault(
    ctx: &Context<Data>,
    ns: &str,
    name: &str,
) -> Result<serde_json::map::Map<String, Value>, Error> {
    let host = &ctx.get_ref().config.host;
    let token = &ctx.get_ref().config.token;
    let client = hashicorp_vault::Client::new(host, token).unwrap();
    let res: hashicorp_vault::client::error::Result<Value> =
        client.get_custom_secret(format!("{}/{}", ns, name));

    Ok(res?
        .as_object()
        .ok_or(ArcanumError {
            reason: "path in vault is not an object!".to_string(),
        })?
        .clone())
}

fn set_in_vault(
    ctx: &Context<Data>,
    ns: &str,
    name: &str,
    data: BTreeMap<String, ByteString>,
) -> Result<(), Error> {
    let host = &ctx.get_ref().config.host;
    let token = &ctx.get_ref().config.token;
    let client = hashicorp_vault::Client::new(host, token).unwrap();
    let data = data
        .iter()
        .map(|x| (x.0, std::str::from_utf8(&*x.1 .0).unwrap()))
        .collect::<BTreeMap<&String, &str>>();
    let res: hashicorp_vault::client::error::Result<()> =
        client.set_custom_secret(format!("{}/{}", ns, name), &data);

    Ok(res?)
}

pub fn object_to_owner_reference<K: Resource<DynamicType = ()>>(
    meta: ObjectMeta,
) -> Result<OwnerReference, Error> {
    Ok(OwnerReference {
        api_version: K::api_version(&()).to_string(),
        kind: K::kind(&()).to_string(),
        name: meta.name.unwrap(),
        uid: meta.uid.unwrap(),
        ..OwnerReference::default()
    })
}

fn decrypt(key: &SecretKey, data: Vec<u8>) -> Result<Vec<u8>, Error> {
    Ok(ecies_ed25519::decrypt(key, &data)?)
}

#[instrument(skip(ctx), fields(trace_id))]
async fn reconcile(foo: SyncedSecret, ctx: Context<Data>) -> Result<ReconcilerAction, Error> {
    let trace_id = telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));
    let start = Instant::now();

    let client = ctx.get_ref().client.clone();
    ctx.get_ref().state.write().await.last_event = Utc::now();
    let name = ResourceExt::name(&foo);
    let ns = ResourceExt::namespace(&foo).expect("syncedsecret is namespaced");
    // let syncedsecrets: Api<SyncedSecret> = Api::namespaced(client.clone(), &ns);
    let secrets: Api<Secret> = Api::namespaced(client, &ns);

    let secret_obj = secrets.get(&name).await;
    let secret_vlt = get_from_vault(&ctx, &ns, &name);
    let keypair: SecretKey =
        SecretKey::from_bytes(&*base64::decode(std::env::var("ARCANUM_ENC_KEY").unwrap()).unwrap()).unwrap();

    let x = match secret_obj {
        Ok(s) => {
            if secret_vlt.is_err() {
                let data = match s.data {
                    None => {
                        let x: BTreeMap<String, ByteString> = BTreeMap::new();
                        x
                    }
                    Some(s) => s,
                };
                set_in_vault(&ctx, &ns, &name, data)?;
            }
            (secret_vlt?, true)
        }
        Err(_) => {
            if secret_vlt.is_ok() {
                (secret_vlt?, true)
            } else {
                let foospec: SyncedSecretSpec = foo.spec;
                let unsealed = foospec
                    .data
                    .iter()
                    .map(|x| {
                        (
                            x.0.clone(),
                            serde_json::Value::String(
                                std::str::from_utf8(&*decrypt(&keypair, base64::decode(x.1).unwrap()).unwrap())
                                    .unwrap()
                                    .parse()
                                    .unwrap(),
                            ),
                        )
                    })
                    .collect();
                (unsealed, false)
            }
        }
    };
    let to_create = x.0;
    let apply_fn = x.1;

    let to_create = to_create
        .iter()
        .map(|x| {
            (
                x.0.clone(),
                ByteString(Vec::from(x.1.as_str().unwrap().as_bytes())),
            )
        })
        .collect::<BTreeMap<String, ByteString>>();

    let owner_reference = object_to_owner_reference::<SyncedSecret>(foo.metadata.clone())?;
    let secret_obj = Secret {
        data: Some(to_create),
        metadata: ObjectMeta {
            name: Some(name.clone()),
            owner_references: Some(vec![owner_reference.clone()]),
            ..ObjectMeta::default()
        },
        // TODO: type_
        // type_: Re,
        ..Secret::default()
    };

    if apply_fn {
        secrets
            .patch(
                &name,
                &PatchParams::apply("arcanum.njha.dev"),
                &Patch::Apply(&secret_obj),
            )
            .await
            .unwrap();
    } else {
        secrets.create(&PostParams::default(), &secret_obj).await.unwrap();
    }

    let duration = start.elapsed().as_millis() as f64 / 1000.0;
    ctx.get_ref()
        .metrics
        .reconcile_duration
        .with_label_values(&[])
        .observe(duration);
    ctx.get_ref().metrics.handled_events.inc();
    info!("Reconciled SyncedSecret \"{}\" in {}", name, ns);

    // If no events were received, check back every 3..5 minutes
    let mut rng = rand::thread_rng();
    Ok(ReconcilerAction {
        requeue_after: Some(Duration::from_secs(rng.gen_range((3*60)..(5*60)))),
    })
}

fn error_policy(error: &Error, _ctx: Context<Data>) -> ReconcilerAction {
    warn!("reconcile failed: {:?}", error);
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(360)),
    }
}

/// Metrics exposed on /metrics
#[derive(Clone)]
pub struct Metrics {
    pub handled_events: IntCounter,
    pub reconcile_duration: HistogramVec,
}

impl Metrics {
    fn new() -> Self {
        let reconcile_histogram = register_histogram_vec!(
            "foo_controller_reconcile_duration_seconds",
            "The duration of reconcile to complete in seconds",
            &[],
            vec![0.01, 0.1, 0.25, 0.5, 1., 5., 15., 60.]
        )
        .unwrap();

        Metrics {
            handled_events: register_int_counter!("foo_controller_handled_events", "handled events").unwrap(),
            reconcile_duration: reconcile_histogram,
        }
    }
}

/// In-memory reconciler state exposed on /
#[derive(Clone, Serialize)]
pub struct State {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}

impl State {
    fn new() -> Self {
        State {
            last_event: Utc::now(),
            reporter: "arcanum".into(),
        }
    }
}

/// Data owned by the Manager
#[derive(Clone)]
pub struct Manager {
    /// In memory state
    state: Arc<RwLock<State>>,
}

/// Example Manager that owns a Controller for Foo
impl Manager {
    /// Lifecycle initialization interface for app
    ///
    /// This returns a `Manager` that drives a `Controller` + a future to be awaited
    /// It is up to `main` to wait for the controller stream.
    pub async fn new() -> (Self, BoxFuture<'static, ()>) {
        let client = Client::try_default().await.expect("create client");
        let metrics = Metrics::new();
        let state = Arc::new(RwLock::new(State::new()));
        let context = Context::new(Data {
            client: client.clone(),
            metrics: metrics.clone(),
            state: state.clone(),
            // confy::load_path(std::env::var("ARCANUM_CONFIG").unwrap_or("/etc/arcanum/config.toml".into())).unwrap()
            config: AppConfig {
                version: 0,
                host: std::env::var("ARCANUM_VLT_HOST").unwrap(),
                token: std::env::var("ARCANUM_VLT_TOKEN").unwrap(),
                path: std::env::var("ARCANUM_VLT_PATH").unwrap(),
            },
        });

        let foos = Api::<SyncedSecret>::all(client);
        // Ensure CRD is installed before loop-watching
        let _r = foos
            .list(&ListParams::default().limit(1))
            .await
            .expect("is the crd installed? please run: cargo run --bin crdgen | kubectl apply -f -");

        // All good. Start controller and return its future.
        let drainer = Controller::new(foos, ListParams::default())
            .run(reconcile, error_policy, context)
            .filter_map(|x| async move { std::result::Result::ok(x) })
            .for_each(|_| futures::future::ready(()))
            .boxed();

        (Self { state }, drainer)
    }

    /// Metrics getter
    pub fn metrics(&self) -> Vec<MetricFamily> {
        default_registry().gather()
    }

    /// State getter
    pub async fn state(&self) -> State {
        self.state.read().await.clone()
    }
}
