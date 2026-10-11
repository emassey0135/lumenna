//! Ordering by urgency, and the weights every device ranks by.

use lumenna_surface::Lumenna;

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn titles(lumenna: &Lumenna, query: &str) -> Vec<String> {
    lumenna.list_tasks(query).unwrap().rows.into_iter().map(|r| r.title).collect()
}

#[test]
fn a_list_sorted_by_urgency_puts_the_most_urgent_first_by_the_synced_weights() {
    let (_directory, lumenna) = open();
    lumenna.add_task("Low p4").unwrap();
    lumenna.add_task("High p1").unwrap();
    assert_eq!(titles(&lumenna, ""), ["Low", "High"], "the person's own order");
    assert_eq!(titles(&lumenna, "sort: urgency"), ["High", "Low"]);
    let rows = lumenna.list_tasks("p1 | p4 sort: urgency").unwrap();
    assert!(rows.query.unwrap().description.ends_with(", most urgent first"));
    // Weights that favour p4 turn the order round.
    lumenna.set_setting("urgency-weights", "priority 0 0 0 6").unwrap();
    assert_eq!(titles(&lumenna, "sort: urgency"), ["Low", "High"]);
    let setting = lumenna.settings(Some("urgency-weights".to_owned())).unwrap().settings.remove(0);
    assert!(setting.syncs);
    assert!(setting.value.contains("priority 0 0 0 6"), "{}", setting.value);
}
