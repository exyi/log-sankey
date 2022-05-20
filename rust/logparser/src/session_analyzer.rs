use std::{collections::{HashMap, hash_map::DefaultHasher, BTreeMap, BTreeSet}, hash::{Hash, Hasher}, pin::Pin, ops::Add};

use chrono::NaiveDateTime;
use futures::{Stream, stream, StreamExt};

use crate::{parser::*, log};

#[derive(Clone, Debug)]
pub struct Session {
	pub ip: u32,
	pub user_agent: u32,
	pub referer: u32,
	pub start_time: NaiveDateTime,
	pub end_time: NaiveDateTime,
	// seconds since startime
	pub access_times: Vec<u32>,
	/// list of html pages (paths) accessed by this session
	pub actions: Vec<u32>,
	pub total_requests: u32,
	pub total_bytes: u64,
}
impl Session {
	pub fn compute_id(ip: u32, user_agent: u32) -> u64 {
		// let mut hasher = DefaultHasher::new();
		// (logline.ip, logline.user_agent).hash(&mut hash);

		((ip as u64) << 32) | user_agent as u64
	}
	pub fn id(&self) -> u64 {
		Session::compute_id(self.ip, self.user_agent)
	}
}

pub fn get_sessions<'a>(
	table: &'a GlobalTable,
	stream: impl Stream<Item=Vec<LogLine>> + 'a,
	max_age: u32
) -> impl Stream<Item=Session> + 'a {
	let mut sessions: HashMap<u64, Session> = HashMap::new();
	let mut session_age: BTreeSet<(NaiveDateTime, u64)> = BTreeSet::new();

	let last_element = vec![ vec![] ];

	// WTF, Rust...
	let tmp: Vec<Pin<Box<dyn Stream<Item=Vec<_>>>>> = vec! [
		Box::pin(stream.filter(|x| futures::future::ready(x.len() > 0))),
		Box::pin(stream::iter(last_element))
	];
	stream::iter(tmp).flat_map(|x| x).flat_map(move |loglines| {
		let loglines_len = loglines.len();
		let mut result: Vec<Session> = vec![];

		for logline in loglines.iter().filter(|&l: &&LogLine|
			l.status_code >= 200 && l.status_code < 300
		) {
			let is_meaningless = table.is_meaningless(logline);
			let session_id = Session::compute_id(logline.ip, logline.user_agent);

			{
				let s = if let Some(s) = sessions.get_mut(&session_id) {
					session_age.remove(&(s.end_time, s.id()));
					s
				} else {
					if is_meaningless {
						// don't create session with meaningless request
						continue;
					}
					let s = Session {
						ip: logline.ip,
						user_agent: logline.user_agent,
						referer: logline.referer,
						end_time: logline.time,
						start_time: logline.time,
						access_times: vec![],
						actions: vec![],
						total_requests: 0,
						total_bytes: 0,
					};
					sessions.insert(session_id, s);
					sessions.get_mut(&session_id).unwrap()
				};

				let mut acctime = logline.time.timestamp() - s.start_time.timestamp();
				if acctime < 0 {
					acctime = 0;
				}
				assert!(acctime >= 0, "acctime is negative, start_time = {}, logtime = {}", s.start_time, logline.time);
				assert!(acctime <= max_age as i64 * 10000);

				let is_meaningless = is_meaningless || (
					table.is_probably_meaningless(logline) &&
						s.actions.len() > 0 &&
						s.end_time.add(chrono::Duration::seconds(10)) > logline.time);

				s.total_requests += 1;
				s.total_bytes += logline.size;

				if !is_meaningless {
					// only track meaningfull actions (not resource loading)
					s.access_times.push(acctime as u32);
					s.end_time = logline.time;
					s.actions.push(logline.path);
					session_age.insert((s.end_time, session_id));
				}

			}

			while let Some(&(time, oldest_session)) = session_age.first() {
				if time.timestamp() >= logline.time.timestamp() - (max_age as i64) {
					break;
				}
				let s = sessions.remove(&oldest_session).unwrap();
				result.push(s);
				session_age.pop_first();
			}
		}
		// log!("{} Sessions purged out of shit", result.len());

		if loglines_len == 0 {
			// last element
			result.extend(sessions.drain().map(|(_, s)| s));
			session_age.clear();
			// log!("[last buffer] {} Sessions purged out of shit", result.len());
		}

		futures::stream::iter(result)
	})
}
