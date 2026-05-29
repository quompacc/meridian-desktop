// Polkit authentication agent — system-bus side.
//
// Implements `org.freedesktop.PolicyKit1.AuthenticationAgent`, registers
// itself with `org.freedesktop.PolicyKit1.Authority` for the current
// `unix-session`, and serializes incoming BeginAuthentication requests
// onto a calloop channel for the UI main loop to handle.
//
// On the way back, `Outcome::Authenticated{uid, identity}` triggers an
// `AuthenticationAgentResponse2(uid, cookie, identity)` call against
// polkitd before BeginAuthentication returns Ok — which is what polkit
// uses to actually grant the action.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::reexports::calloop::channel as cchannel;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};
use zbus::{interface, proxy, zvariant::OwnedValue, zvariant::Value, Connection};

pub const AGENT_OBJECT_PATH: &str = "/org/freedesktop/PolicyKit1/AuthenticationAgent";

const POLKIT_BUS_NAME: &str = "org.freedesktop.PolicyKit1";
const POLKIT_OBJECT_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";

/// One polkit identity (we only support unix-user; unix-group is rare
/// for end-user auth flows).
#[derive(Clone, Debug)]
pub struct Identity {
    pub uid: u32,
    pub username: String,
}

/// A pending auth request, forwarded from the dbus thread to the UI loop.
pub struct AuthRequest {
    pub action_id: String,
    pub message: String,
    pub icon_name: String,
    pub details: HashMap<String, String>,
    pub cookie: String,
    pub identities: Vec<Identity>,
    /// UI loop sends the user's outcome back here. Dropping it (or sending
    /// `Cancelled`) makes BeginAuthentication return without granting.
    pub reply: oneshot::Sender<Outcome>,
}

/// Result of a single BeginAuthentication round.
pub enum Outcome {
    /// PAM succeeded for the given identity. The agent will call
    /// AuthenticationAgentResponse2 with these values.
    Authenticated { uid: u32, username: String },
    /// User dismissed or PAM failed after retries. polkitd treats this
    /// as "not authorised".
    Cancelled,
}

/// Cancellations from polkit (CancelAuthentication) are forwarded as
/// the bare cookie; UI maps it to whichever popup is currently showing.
pub enum DbusEvent {
    BeginAuth(AuthRequest),
    Cancel { cookie: String },
}

#[derive(Clone)]
struct AgentService {
    tx: cchannel::Sender<DbusEvent>,
    /// cookie -> oneshot tx (used by CancelAuthentication to wake the
    /// pending BeginAuthentication future)
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
}

#[interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl AgentService {
    /// polkitd → agent. We forward to the UI loop and await its reply
    /// via a oneshot. If the user authenticates, we call
    /// AuthenticationAgentResponse2 on polkitd BEFORE returning.
    async fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        icon_name: String,
        details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedValue>)>,
    ) -> zbus::fdo::Result<()> {
        let ids = parse_identities(&identities);
        if ids.is_empty() {
            warn!(action_id, cookie, "polkit: no usable identities, declining");
            return Ok(());
        }
        info!(
            action_id = %action_id,
            cookie = %cookie,
            identities = ids.len(),
            "polkit: BeginAuthentication received"
        );

        let (reply_tx, reply_rx) = oneshot::channel();
        let (cancel_tx, mut cancel_rx) = oneshot::channel();

        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(cookie.clone(), cancel_tx);
        }

        let req = AuthRequest {
            action_id,
            message,
            icon_name,
            details,
            cookie: cookie.clone(),
            identities: ids,
            reply: reply_tx,
        };
        if let Err(e) = self.tx.send(DbusEvent::BeginAuth(req)) {
            warn!(error = ?e, "polkit: UI channel closed; declining auth");
            self.pending.lock().unwrap().remove(&cookie);
            return Ok(());
        }

        let outcome = tokio::select! {
            r = reply_rx => r.unwrap_or(Outcome::Cancelled),
            _ = &mut cancel_rx => Outcome::Cancelled,
        };

        self.pending.lock().unwrap().remove(&cookie);

        // No Response2 call here: the setuid `polkit-agent-helper-1`
        // that did PAM also called Response on polkitd as root. We just
        // return — the BeginAuthentication reply tells polkitd we're
        // done. On Cancel we still return Ok(()) without the helper
        // having called Response, so polkitd treats the action as not
        // authorised.
        let _ = outcome;
        Ok(())
    }

    /// polkitd → agent. Wake the matching BeginAuthentication future so
    /// it returns; also tell the UI to close its popup.
    async fn cancel_authentication(&self, cookie: String) -> zbus::fdo::Result<()> {
        debug!(cookie = %cookie, "polkit: CancelAuthentication");
        if let Some(tx) = self.pending.lock().unwrap().remove(&cookie) {
            let _ = tx.send(());
        }
        let _ = self.tx.send(DbusEvent::Cancel { cookie });
        Ok(())
    }
}

#[proxy(
    interface = "org.freedesktop.PolicyKit1.Authority",
    default_service = "org.freedesktop.PolicyKit1",
    default_path = "/org/freedesktop/PolicyKit1/Authority"
)]
trait Authority {
    fn register_authentication_agent(
        &self,
        subject: &(String, HashMap<String, Value<'_>>),
        locale: &str,
        object_path: &str,
    ) -> zbus::Result<()>;

    fn unregister_authentication_agent(
        &self,
        subject: &(String, HashMap<String, Value<'_>>),
        object_path: &str,
    ) -> zbus::Result<()>;
}

fn unix_session_subject(session_id: &str) -> (String, HashMap<String, Value<'static>>) {
    let mut details: HashMap<String, Value<'static>> = HashMap::new();
    details.insert("session-id".to_string(), Value::from(session_id.to_string()));
    ("unix-session".to_string(), details)
}

fn parse_identities(
    raw: &[(String, HashMap<String, OwnedValue>)],
) -> Vec<Identity> {
    raw.iter()
        .filter_map(|(kind, details)| {
            if kind != "unix-user" {
                return None;
            }
            let uid = details.get("uid").and_then(|v| u32::try_from(v).ok())?;
            let username = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(uid))
                .ok()
                .flatten()
                .map(|u| u.name)
                .unwrap_or_else(|| format!("uid={uid}"));
            Some(Identity { uid, username })
        })
        .collect()
}

/// Spawn the polkit agent on its own OS thread (single-thread tokio
/// runtime, same pattern as meridian-shell's notifications daemon).
/// Returns the calloop receiver end that the UI loop should drain.
///
/// `session_id` is the polkit subject id — typically `XDG_SESSION_ID`.
pub fn spawn(session_id: String, locale: String) -> std::io::Result<cchannel::Channel<DbusEvent>> {
    let (tx, rx) = cchannel::channel::<DbusEvent>();
    std::thread::Builder::new()
        .name("polkit-dbus".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    error!(error = %e, "polkit: tokio runtime build failed; agent disabled");
                    return;
                }
            };
            rt.block_on(async move {
                if let Err(e) = run(tx, session_id, locale).await {
                    error!(error = %e, "polkit: dbus serve failed; agent disabled");
                }
            });
        })?;
    Ok(rx)
}

async fn run(
    tx: cchannel::Sender<DbusEvent>,
    session_id: String,
    locale: String,
) -> zbus::Result<()> {
    let service = AgentService {
        tx,
        pending: Arc::new(Mutex::new(HashMap::new())),
    };

    // Session bus would not be reachable by polkitd (it lives on the
    // system bus). Instead we serve the agent interface on the SYSTEM
    // bus too — polkit calls us back via the unique bus name it sees
    // during RegisterAuthenticationAgent.
    let conn = zbus::connection::Builder::system()?
        .serve_at(AGENT_OBJECT_PATH, service)?
        .build()
        .await?;

    let proxy = AuthorityProxy::new(&conn).await?;
    let subject = unix_session_subject(&session_id);
    proxy
        .register_authentication_agent(&subject, &locale, AGENT_OBJECT_PATH)
        .await?;
    info!(
        bus_name = %conn.unique_name().map(|n| n.to_string()).unwrap_or_default(),
        session_id = %session_id,
        locale = %locale,
        path = AGENT_OBJECT_PATH,
        POLKIT_BUS_NAME,
        POLKIT_OBJECT_PATH,
        "polkit: agent registered"
    );

    // Park forever. Unregistration on shutdown would be nicer; for now
    // polkitd reaps stale agents when the bus name disappears.
    std::future::pending::<()>().await;
    Ok(())
}
