use gtk::glib::object::Cast;
use webkit2gtk::{
    NavigationPolicyDecision, NavigationPolicyDecisionExt, PolicyDecisionExt, PolicyDecisionType,
    SettingsExt, URIRequestExt, WebView, WebViewExt,
};

use crate::document::is_allowed_top_level_uri;

pub(super) fn harden_settings(settings: &webkit2gtk::Settings) {
    settings.set_enable_javascript(true);
    settings.set_enable_developer_extras(false);
    settings.set_enable_dns_prefetching(false);
    settings.set_enable_html5_database(false);
    settings.set_enable_html5_local_storage(false);
    settings.set_enable_media_stream(false);
    settings.set_javascript_can_access_clipboard(false);
    settings.set_javascript_can_open_windows_automatically(false);
}

pub(super) fn install_navigation_policy(webview: &WebView) {
    webview.connect_decide_policy(|_, decision, decision_type| {
        if decision_type == PolicyDecisionType::NewWindowAction {
            decision.ignore();
            return true;
        }
        if decision_type != PolicyDecisionType::NavigationAction {
            return false;
        }
        let Some(navigation) = decision.dynamic_cast_ref::<NavigationPolicyDecision>() else {
            decision.ignore();
            return true;
        };
        let uri = navigation
            .navigation_action()
            .and_then(|action| action.request())
            .and_then(|request| request.uri());
        if uri.as_deref().is_some_and(is_allowed_top_level_uri) {
            return false;
        }
        eprintln!("meridian-ui-runtime: denied top-level navigation to {uri:?}");
        decision.ignore();
        true
    });
}
