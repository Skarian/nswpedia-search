use nswpedia_search::resolver::{Cancel, ResolveEvent, Resolver};
use std::{sync::Mutex, time::Instant};

fn main() -> anyhow::Result<()> {
    let query = std::env::args().nth(1).unwrap_or_else(|| "Celeste".into());
    let cache = std::env::args()
        .nth(2)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("nswpedia-native-check.sqlite"));
    let resolver = Resolver::new(&cache)?;
    let start = Instant::now();
    let cancel = Cancel::default();
    let search = resolver.search(&query, 1, &cancel)?;
    let game = search
        .games
        .first()
        .ok_or_else(|| anyhow::anyhow!("No results"))?;
    let first = Mutex::new(None);
    let links = Mutex::new(Vec::new());
    let errors = Mutex::new(Vec::new());
    resolver.resolve(game, 0, &cancel, &|event| match event {
        ResolveEvent::Link { key, url } => {
            first
                .lock()
                .unwrap()
                .get_or_insert(start.elapsed().as_millis());
            links
                .lock()
                .unwrap()
                .push(serde_json::json!({"key": key,"url": url}));
        }
        ResolveEvent::Error { message, .. } => errors.lock().unwrap().push(message),
        _ => {}
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "query":query,"game":game,"results":search.games.len(),
            "first_link_ms":*first.lock().unwrap(),"total_ms":start.elapsed().as_millis(),
            "links":*links.lock().unwrap(),"errors":*errors.lock().unwrap()
        }))?
    );
    anyhow::ensure!(errors.lock().unwrap().is_empty(), "Link resolution failed");
    anyhow::ensure!(!links.lock().unwrap().is_empty(), "No links resolved");
    Ok(())
}
