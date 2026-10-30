//! The dataset is deterministic, synthetic and valid through the core.

use tasknotes_bench::dataset;
use tasknotes_bench::measure;
use tasknotes_core::tasks::TaskStore;

#[test]
fn the_same_seed_gives_the_same_content_and_sizes_change_it() {
    assert_eq!(dataset::fingerprint(200, 50), dataset::fingerprint(200, 50));
    assert_ne!(dataset::fingerprint(200, 50), dataset::fingerprint(201, 50));
}

#[test]
fn a_small_dataset_seeds_through_the_core_and_every_scenario_runs() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dataset::seed(dir.path(), 1500, 300).unwrap();
    assert_eq!(manifest["tasks"], 1500);
    assert_eq!(manifest["notes"], 300);
    let store = TaskStore::open(&dir.path().join("tasknotes.db")).unwrap();
    for scenario in measure::scenarios(&manifest) {
        measure::run(&store, &scenario.query);
    }
    // Pages stay pages: nothing returns more than was asked for.
    let page = store
        .query_tasks(&Default::default(), measure::PAGE, None)
        .unwrap();
    assert_eq!(page.items.len(), measure::PAGE as usize);
    assert!(page.next_cursor.is_some());
}

#[test]
fn percentiles_are_nearest_rank() {
    let samples: Vec<f64> = (1..=100).map(f64::from).collect();
    assert_eq!(measure::percentile(&samples, 95), 95.0);
    assert_eq!(measure::percentile(&samples, 50), 50.0);
    assert_eq!(measure::percentile(&[7.0], 95), 7.0);
}
