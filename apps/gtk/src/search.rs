//! The desktop's own search, answered from the core's (`lumenna_records::search`): GNOME Shell's
//! search provider (`org.gnome.Shell.SearchProvider2`) and KDE's KRunner (`org.kde.krunner1`),
//! both objects on the app's own connection to the session bus. What matches, in what order, and
//! how a result reads (its title and line) are the core's (`Device::search_results`); a result
//! chosen is handed back to the app as its `launch` (`window::launch`), which opens it.
//!
//! The shell and KRunner find the app by its files under `data/`: the provider's `.ini` and the
//! runner's `.desktop`, and a D-Bus service file that starts the app in the background to answer
//! when it is not running.

use std::cell::RefCell;
use std::collections::HashMap;

use gtk::gio;
use gtk::glib::{self, Variant, VariantTy};
use gtk::prelude::*;
use lumenna_records::search::SearchDocument;

/// Where GNOME Shell asks, as the provider's `.ini` names it.
pub const SEARCH_PROVIDER_PATH: &str = "/dev/emassey0135/Lumenna/SearchProvider";
/// Where KRunner asks, as the runner's `.desktop` names it.
pub const KRUNNER_PATH: &str = "/dev/emassey0135/Lumenna/KRunner";

const INTERFACES: &str = r#"<node>
  <interface name="org.gnome.Shell.SearchProvider2">
    <method name="GetInitialResultSet">
      <arg type="as" name="terms" direction="in"/>
      <arg type="as" name="results" direction="out"/>
    </method>
    <method name="GetSubsearchResultSet">
      <arg type="as" name="previous_results" direction="in"/>
      <arg type="as" name="terms" direction="in"/>
      <arg type="as" name="results" direction="out"/>
    </method>
    <method name="GetResultMetas">
      <arg type="as" name="identifiers" direction="in"/>
      <arg type="aa{sv}" name="metas" direction="out"/>
    </method>
    <method name="ActivateResult">
      <arg type="s" name="identifier" direction="in"/>
      <arg type="as" name="terms" direction="in"/>
      <arg type="u" name="timestamp" direction="in"/>
    </method>
    <method name="LaunchSearch">
      <arg type="as" name="terms" direction="in"/>
      <arg type="u" name="timestamp" direction="in"/>
    </method>
  </interface>
  <interface name="org.kde.krunner1">
    <method name="Actions">
      <arg type="a(sss)" name="matches" direction="out"/>
    </method>
    <method name="Config">
      <arg type="a{sv}" name="config" direction="out"/>
    </method>
    <method name="Match">
      <arg type="s" name="query" direction="in"/>
      <arg type="a(sssida{sv})" name="matches" direction="out"/>
    </method>
    <method name="Run">
      <arg type="s" name="matchId" direction="in"/>
      <arg type="s" name="actionId" direction="in"/>
    </method>
    <method name="Teardown"/>
  </interface>
</node>"#;

/// KRunner's category relevance for every result: a possible match (KF5's `PossibleMatch`, KF6's
/// `Low`), since the words were looked for among a person's tasks, not named exactly.
const KRUNNER_CATEGORY: i32 = 30;

thread_local! {
    /// What the last searches found, by identifier, for the shell's `GetResultMetas` and an
    /// activation, which name a result by its identifier alone.
    static FOUND: RefCell<HashMap<String, SearchDocument>> = RefCell::new(HashMap::new());
}

/// Puts both objects on the application's connection, as it registers.
pub fn export(application: &gtk::Application) {
    let Some(connection) = application.dbus_connection() else { return };
    let node = match gio::DBusNodeInfo::for_xml(INTERFACES) {
        Ok(node) => node,
        Err(error) => {
            eprintln!("Lumenna's search interfaces did not read: {error}");
            return;
        }
    };
    for (path, interface) in [(SEARCH_PROVIDER_PATH, "org.gnome.Shell.SearchProvider2"), (KRUNNER_PATH, "org.kde.krunner1")] {
        let Some(info) = node.lookup_interface(interface) else { continue };
        let registered = connection
            .register_object(path, &info)
            .method_call(|_, _, _, interface, method, parameters, invocation| {
                let answer = match interface {
                    Some("org.gnome.Shell.SearchProvider2") => gnome(method, &parameters),
                    Some("org.kde.krunner1") => krunner(method, &parameters),
                    _ => None,
                };
                invocation.return_value(answer.as_ref());
            })
            .build();
        if let Err(error) = registered {
            eprintln!("Lumenna's search could not be offered at {path}: {error}");
        }
    }
}

/// What the core finds for `text`, kept for the calls that name a result by its identifier.
fn search(text: &str) -> Vec<SearchDocument> {
    let Some(app) = crate::window::app() else { return Vec::new() };
    let found = app.model.device.search_results(text.to_owned()).unwrap_or_default();
    FOUND.with_borrow_mut(|kept| {
        // Kept across searches as the shell narrows one, and forgotten once there are many.
        if kept.len() > 1000 {
            kept.clear();
        }
        kept.extend(found.iter().map(|document| (document.id.clone(), document.clone())));
    });
    found
}

/// A result found before, by its identifier; else looked for among everything the search holds.
fn found(id: &str) -> Option<SearchDocument> {
    if let Some(document) = FOUND.with_borrow(|kept| kept.get(id).cloned()) {
        return Some(document);
    }
    let app = crate::window::app()?;
    app.model.device.search_documents().ok()?.into_iter().find(|document| document.id == id)
}

/// Opens a result chosen, as the core's launch says.
fn open(id: &str) {
    if let Some(document) = found(id) {
        crate::window::launch(&document.launch);
    }
}

fn identifiers(found: &[SearchDocument]) -> Variant {
    (found.iter().map(|document| document.id.clone()).collect::<Vec<_>>(),).to_variant()
}

/// The app's icon, as either search names an icon.
fn icon() -> String {
    crate::ID.to_owned()
}

fn gnome(method: &str, parameters: &Variant) -> Option<Variant> {
    match method {
        "GetInitialResultSet" => {
            let (terms,) = parameters.get::<(Vec<String>,)>()?;
            Some(identifiers(&search(&terms.join(" "))))
        }
        "GetSubsearchResultSet" => {
            let (_, terms) = parameters.get::<(Vec<String>, Vec<String>)>()?;
            Some(identifiers(&search(&terms.join(" "))))
        }
        "GetResultMetas" => {
            let (ids,) = parameters.get::<(Vec<String>,)>()?;
            let metas: Vec<Variant> = ids
                .iter()
                .filter_map(|id| found(id))
                .map(|document| {
                    let meta = glib::VariantDict::new(None);
                    meta.insert("id", &document.id);
                    meta.insert("name", &document.title);
                    meta.insert("description", &document.line);
                    meta.insert("gicon", icon());
                    meta.end()
                })
                .collect();
            Some(Variant::tuple_from_iter([Variant::array_from_iter_with_type(VariantTy::VARDICT, metas)]))
        }
        "ActivateResult" => {
            let (id, _, _) = parameters.get::<(String, Vec<String>, u32)>()?;
            open(&id);
            None
        }
        // The search's own button: the app, shown.
        "LaunchSearch" => {
            if let Some(app) = crate::window::app() {
                app.show_window();
            }
            None
        }
        _ => None,
    }
}

fn krunner(method: &str, parameters: &Variant) -> Option<Variant> {
    match method {
        "Actions" => Some(Variant::tuple_from_iter([Variant::array_from_iter_with_type(
            VariantTy::new("(sss)").expect("a type"),
            Vec::<Variant>::new(),
        )])),
        "Config" => Some(Variant::tuple_from_iter([glib::VariantDict::new(None).end()])),
        "Match" => {
            let (query,) = parameters.get::<(String,)>()?;
            let found = search(&query);
            let count = found.len().max(1) as f64;
            let matches: Vec<Variant> = found
                .iter()
                .enumerate()
                .map(|(place, document)| {
                    let properties = glib::VariantDict::new(None);
                    properties.insert("subtext", &document.line);
                    // KRunner orders by relevance; the core's order is most fitting first.
                    let relevance = 1.0 - place as f64 / count;
                    Variant::tuple_from_iter([
                        document.id.to_variant(),
                        document.title.to_variant(),
                        icon().to_variant(),
                        KRUNNER_CATEGORY.to_variant(),
                        relevance.to_variant(),
                        properties.end(),
                    ])
                })
                .collect();
            Some(Variant::tuple_from_iter([Variant::array_from_iter_with_type(
                VariantTy::new("(sssida{sv})").expect("a type"),
                matches,
            )]))
        }
        "Run" => {
            let (id, _) = parameters.get::<(String, String)>()?;
            open(&id);
            None
        }
        _ => None,
    }
}
