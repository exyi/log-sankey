use std::collections::{HashMap, HashSet};

use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{session_analyzer::Session, parser::GlobalTable};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct StatsOptions {
    pub resolution_sec: u32,
    pub threshold: u32,
    pub max_paths: u32,
}
#[wasm_bindgen]
impl StatsOptions {
    #[wasm_bindgen(constructor)]
    pub fn new(resolution_sec: u32, threshold: u32, max_paths: u32) -> StatsOptions {
        StatsOptions { resolution_sec, threshold, max_paths }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStatRow {
    pub category: String,
    pub count: Vec<u32>,
    pub time: Vec<u32>
}

// #[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    /// vector of (time, count)
    pub rows: Vec<UsageStatRow>,
    pub start_time: i64,
    pub end_time: i64,
    pub session_starts_only: bool
}
impl UsageStats {
}
pub fn make_inverse_core<'a>(mapping: &'a HashMap<String, u32>, default: &'a str) -> Vec<&'a str> {
    let path_len = *mapping.values().max().unwrap() as usize + 1;
    let mut all_keys = vec![default; path_len];
    for (p, &i) in mapping.iter() {
        all_keys[i as usize] = p;
    }

    all_keys
}
pub fn make_inverse_mapping<'a>(mapping: &'a HashMap<String, u32>, default: &'a str) -> impl Fn(&u32) -> String + 'a {
    let all_keys = make_inverse_core(mapping, default);
    move |&i| all_keys[i as usize].to_owned()
}

fn calc_usage_table<Key>(
	sessions: &Vec<Session>,
	all_actions: bool,
    get_property: impl Fn(&Session, usize) -> Key,
	resolution_sec: u32,
) -> HashMap<Key, HashMap<i64, u32>>
	where Key: Sized + Eq + std::hash::Hash + Clone {
	let mut usage_table: HashMap<Key, HashMap<i64, u32>> = HashMap::new();

	for s in sessions.iter() {
		assert!(s.actions.len() > 0);

		let actions_range = if all_actions { 0..s.actions.len() } else { 0..1 };

		
		for (key, &time) in actions_range.map(|i| get_property(s, i)).zip(s.access_times.iter()) {
			let time = s.start_time.timestamp() + time as i64;
			// clamp to resolution
			let time = time / resolution_sec as i64;
			if !usage_table.contains_key(&key) {
				usage_table.insert(key.clone(), HashMap::new());
			}
			let time_table = usage_table.get_mut(&key).unwrap();
			time_table.insert(time, time_table.get(&time).unwrap_or(&0) + 1);
		}	}

	usage_table
}

pub fn calc_stats<Key>(
    sessions: &Vec<Session>,
    opt: &StatsOptions,
    all_actions: bool,
    get_property: impl Fn(&Session, usize) -> Key,
    describe_key: impl Fn(&Key) -> String
) -> UsageStats
    where Key: Sized + Eq + std::hash::Hash + Clone {

	let usage_table = calc_usage_table(sessions, all_actions, get_property, opt.resolution_sec);

    // mapping path -> time -> count
    if usage_table.is_empty() {
        return UsageStats { rows: vec![], start_time: 0, end_time: 0, session_starts_only: all_actions };
    }

    let min_time = usage_table.values().flat_map(|x| x.keys()).map(|&t| t).min().unwrap();
    let max_time = usage_table.values().flat_map(|x| x.keys()).map(|&t| t).max().unwrap();
    log!("min_time: {}, max_time: {}, resolution: {}", min_time, max_time, opt.resolution_sec);

    // path, count, ordered by count
    let mut usage_table_sum: Vec<(Key, u32)> =
        usage_table.iter().map(|(key, time_table)|
            (key.clone(), time_table.values().sum::<u32>())
        ).filter(|&(_, count)| count >= opt.threshold).collect();
    usage_table_sum.sort_unstable_by_key(|&(_, count)| -(count as i64));

    // remove elements beyond max_paths
    usage_table_sum.truncate(opt.max_paths as usize);
    

    let rows: Vec<UsageStatRow> = usage_table_sum.iter().map(|(key, _)| {
        let time_table = &usage_table[key];
        let mut x: Vec<_> = time_table.iter().map(|(time, &c)| ((time - min_time) as u32, c)).collect();
        x.sort_unstable_by_key(|&(time, _)| time);
        let (time, count): (Vec<u32>, Vec<u32>) = x.into_iter().unzip();
        UsageStatRow { category: describe_key(key), count, time }
    }).collect();

    UsageStats { rows, start_time: min_time, end_time: max_time, session_starts_only: all_actions }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionGraphNode {
	pub path: String,
	pub path_id: u32,
	pub session_count: u32,
	pub median_view_time: u32,
	pub drop_count: u32,
	pub transfer_count: HashMap<usize, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionGraphLayer {
	pub nodes: Vec<TransitionGraphNode>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionGraph {
	pub layers: Vec<TransitionGraphLayer>
}

fn dedupe_actions(s: &mut Session) {
	let mut actions: Vec<_> = s.actions.iter().zip(s.access_times.iter()).collect();
	actions.dedup_by_key(|(&action, _time)| action);
	let (actions2, times2) = actions.into_iter().unzip();
	s.actions = actions2;
	s.access_times = times2;
}

fn replace_actions(s: &mut Session, replacement_table: &HashMap<u32, u32>) -> u32 {
	let mut replacements = 0;
	for x in &mut s.actions {
		if let Some(&replacement) = replacement_table.get(&x) {
			*x = replacement;
			replacements += 1;
		}
	}
	replacements
}

fn get_global_replacement_table(table: &GlobalTable) ->  HashMap<u32, u32> {
	let mut t: HashMap<u32, u32> = HashMap::new();

	for (i, path) in table.path_list.iter().enumerate() {
		if path.starts_with("/priv") || path.starts_with("/admin") {
			t.insert(i as u32, *table.path.get("admin").unwrap());
		}

		if path.ends_with(".css") {
			t.insert(i as u32, *table.path.get("css").unwrap());
		}
		if path.ends_with(".js") {
			t.insert(i as u32, *table.path.get("js").unwrap());
		}

		if path.ends_with("/index.html") {
			t.insert(i as u32, *table.path.get(&path[0..path.len() - 11]).unwrap());
		}
	}

	t
}

fn get_usage_table_sum<'a>(sessions: &Vec<Session>, path_idx: &Vec<&'a str>, threshold: u32, max_paths: u32) -> Vec<(&'a str, u32)> {
	let mut usage_table = vec![0u32; path_idx.len()];

	for s in sessions {
		for &a in &s.actions {
			usage_table[a as usize] += 1;
		}
	}


	let mut usage_table_sum: Vec<(&'a str, u32)> =
		usage_table.iter().enumerate()
			.filter(|&(_, &c)| c >= threshold)
			.map(|(i, &c)| (path_idx[i], c)).collect();

	usage_table_sum.sort_unstable_by_key(|&(_, count)| -(count as i64));
	usage_table_sum.truncate(max_paths as usize);

	usage_table_sum
}

pub fn reduce_sessions<'a>(mut sessions: Vec<Session>, table: &'a GlobalTable, threshold: u32, max_paths: u32) -> (Vec<Session>, Vec<(&'a str, u32)>) {
	let path_idx = make_inverse_core(&table.path, "");

	for iteration in 0..1000 {
		let mut usage_table_sum = get_usage_table_sum(&sessions, &path_idx, threshold, max_paths);

		let whitelisted_paths: HashSet<_> = usage_table_sum.iter().map(|&(path, _)| path).collect();
		let existing_paths: HashSet<u32> = sessions.iter().flat_map(|s| s.actions.iter().map(|&a| a)).collect();
		// strip the longest paths first
		let max_path_length = existing_paths.iter()
			.map(|&p| path_idx[p as usize])
			.filter(|p| !whitelisted_paths.contains(p))
			.map(|p| p.matches("/").count())
			.max()
			.unwrap_or(2);

		let replacement_table: HashMap<u32, u32> =
			existing_paths.into_iter()
				.filter(|&p| !whitelisted_paths.contains(&path_idx[p as usize]))
				.filter_map(|p| {
					let path = path_idx[p as usize];
					let path_len = path.matches("/").count();
					if path_len < max_path_length || path_len <= 1 {
						None
					} else {
						if let Some(last_slash) = path.rfind('/') {
							let trimmed_path = &path[0..last_slash];
							assert!(!trimmed_path.ends_with("/"));
							assert!(trimmed_path.starts_with("/"));
							Some((p, *table.path.get(trimmed_path).unwrap()))
						} else {
							None
						}
					}
				})
				.collect();


		let mut replacements = 0;

		for s in &mut sessions {
			replacements += replace_actions(s, &replacement_table);
		}

		if replacements == 0 || iteration == 999 {
			if iteration == 999 {
				log!("Warning: reached maximum iterations, still done {} replacements", replacements);
			}
			usage_table_sum.push(("Rest", 0));
			return (sessions, usage_table_sum);
		}
	}


	unreachable!()
}

pub fn calc_graph(
	sessions: &Vec<Session>,
	table: &GlobalTable,
	graph_length: usize,
	opt: &StatsOptions,
	must_contain: &str,
	must_start_with: &str
) -> TransitionGraph {
	let replacement_table = get_global_replacement_table(table);

	let contains_filter: HashSet<u32> = table.path.iter().filter(|&(k, _)| k.contains(must_contain)).map(|(_, &id)| id).collect();
	let starts_filter: HashSet<u32> = table.path.iter().filter(|&(k, _)| k.contains(must_start_with)).map(|(_, &id)| id).collect();

	let sessions: Vec<Session> =
		sessions.iter().filter_map(|s| {
			let mut s = s.clone();
			if let Some(x) = s.actions.iter().position(|a| starts_filter.contains(a)) {
				s.actions.drain(0..x);
				dedupe_actions(&mut s);
				replace_actions(&mut s, &replacement_table);
				Some(s)
			} else {
				None
			}
		}).filter(|s| s.actions.iter().any(|a| contains_filter.contains(a))).collect();

	let (sessions, usage_table_sum) = reduce_sessions(sessions, &table, opt.threshold, opt.max_paths);

	let nodes: Vec<TransitionGraphNode> =
		usage_table_sum.iter().map(|&(path, _count)| {
			TransitionGraphNode {
				path: path.to_owned(),
				path_id: *table.path.get(path).unwrap_or(&0),
				session_count: 0,
				median_view_time: 0,
				drop_count: 0,
				transfer_count: HashMap::new()
			}
		}).collect();
	let rest_node_index = nodes.len() - 1;
	let node_index: HashMap<u32, usize> = nodes.iter().enumerate().map(|(i, n)| (n.path_id, i)).collect();
	let get_node_index = |path_id: u32| *node_index.get(&path_id).unwrap_or(&rest_node_index);
	let mut layers = vec![ TransitionGraphLayer { nodes }; graph_length ];

	for i in 0..graph_length {
		let layer = &mut layers[i];

		let mut visit_times = vec![ vec![]; layer.nodes.len()];

		for s in &sessions {
			if s.actions.len() <= 1 || s.total_requests < 7 || s.actions.len() > 40 {
				// avoid scrapers and single-action sessions
				continue;
			}
			if s.actions.len() <= i {
				continue;
			}

			let path: u32 = s.actions[i];
			let acc_time: u32 = s.access_times[i];
			
			if let Some(visit_time) = s.access_times.get(i + 1).map(|x| x - acc_time) {
				visit_times[get_node_index(path)].push(visit_time);
			}
			
			let node = &mut layer.nodes[get_node_index(path)];
			node.session_count += 1;
			if let Some(&next_action) = s.actions.get(i + 1) {
				node.transfer_count.entry(get_node_index(next_action)).and_modify(|x| *x += 1).or_insert(1);
			} else {
				node.drop_count += 1;
			}
		}

		for (i, n) in layer.nodes.iter_mut().enumerate() {
			if !visit_times[i].is_empty() {
				visit_times[i].sort_unstable();
				n.median_view_time = visit_times[i][visit_times[i].len() / 2];
			}
		}
	}

	TransitionGraph { layers }
}
