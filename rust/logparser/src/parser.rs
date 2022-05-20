use std::{collections::HashMap, str::FromStr};
use crate::{log};
use regex::{Regex};
use chrono::prelude::*;
use wasm_bindgen::UnwrapThrowExt;

// 2021-05-01 02:16:15 "1.1.1.1" "HTTP/1.0" GET ksp.mff.cuni.cz "/img/home.png" 304 0 0 "https://ksp.mff.cuni.cz/css/@a4d399ff6702838c/ksp.css" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.93 Safari/537.36" "-" 61647 "-" "-"

// default order:
// 1. datetime
// 2. ip
// 3. http version
// 4. method
// 5. domain
// 6. path
// 7. status code
// 8. size
// 9. some number, IDK
// 10. referer
// 11. user agent
// 12. some string, IDK
// 13. some number, IDK
// 14. content type
// 15. compression type

pub struct LogParser {
	regex: Regex,
	capture_idxs: Vec<usize>,
	datetime_format: String,
	ignore_query_string: bool
}

impl LogParser {
	fn strip_query_string<'a>(&self, s: &'a str) -> &'a str {
		if self.ignore_query_string {
			if let Some(idx) = s.find('?') {
				&s[0..idx]
			} else {
				s
			}
		} else {
			s
		}
	}
	fn strip_query_string_u8<'a>(&self, s: &'a[u8]) -> &'a[u8] {
		if self.ignore_query_string {
			if let Some(idx) = findidx(s, b'?') {
				&s[0..idx]
			} else {
				s
			}
		} else {
			s
		}
	}
}

pub fn create_parser(
	pattern: &str,
	capture_idxs: Vec<usize>,
	datetime_format: &str,
	ignore_query_string: bool
) -> LogParser {
	log!("pattern = {}", pattern);
	assert_eq!(capture_idxs.len(), 15, "capture_idxs must have 15 elements");
	let regex = Regex::new(pattern).expect("Could not create regex");
	let max_c = capture_idxs.iter().max().unwrap();
	if (regex.captures_len() as usize) < *max_c {
		panic!("capture_idxs must be smaller than capture groups in regex, found {}, max(capture_idx)={}", regex.captures_len(), *max_c);
	}
	LogParser { regex, capture_idxs, datetime_format: datetime_format.to_owned(), ignore_query_string }
}

pub struct GlobalTable {
	pub ip: HashMap<String, u32>,
	pub http_version: HashMap<String, u32>,
	pub method: HashMap<String, u32>,
	pub domain: HashMap<String, u32>,
	pub path: HashMap<String, u32>,
	pub path_list: Vec<String>,
	pub referer: HashMap<String, u32>,
	pub user_agent: HashMap<String, u32>,
	pub content_type: HashMap<String, u32>,
	pub compression_type: HashMap<String, u32>,
}

impl GlobalTable {
	pub fn new() -> GlobalTable {
		let content_type =
			vec![
				"".to_owned(),
				"text/html".to_owned(),
				"text/css".to_owned(),
				"text/javascript".to_owned(),
				"application/javascript".to_owned(),
				"text/json".to_owned(),
				"application/json".to_owned(),
				"image/png".to_owned(),
				"image/jpeg".to_owned(),
				"image/gif".to_owned(),
				"image/svg+xml".to_owned(),
			].into_iter().enumerate().map(|(a, b)| (b, a as u32)).collect();
		let paths = vec![
			"/",
			"/index.html",
			"css",
			"js",
			"img",
			"rest",
			"api",
			"admin"
		];
		GlobalTable {
			ip: HashMap::new(),
			http_version: HashMap::new(),
			method: HashMap::new(),
			domain: HashMap::new(),
			path: paths.iter().enumerate().map(|(i, &x)| (x.to_owned(), i as u32)).collect(),
			path_list: paths.iter().map(|&x| x.to_owned()).collect(),
			referer: HashMap::new(),
			user_agent: HashMap::new(),
			content_type,
			compression_type: HashMap::new(),
		}
	}

	pub fn add_path(&mut self, path: &str) -> u32 {
		if path.as_bytes().last().cloned() == Some(b'/') {
			return self.add_path(&path[0..path.len()-1]);
		}

		if let Some(&idx) = self.path.get(path) {
			idx
		} else {
			// also add all subpaths

			let last_slash = path.rfind('/').unwrap_or(0);
			if last_slash > 0 {
				self.add_path(&path[0..last_slash]);
			}


			let idx = self.path_list.len() as u32;
			self.path_list.push(path.to_owned());
			self.path.insert(path.to_string(), idx);
			idx
		}

	}

	pub fn is_meaningless(&self, l: &LogLine) -> bool {
		let t = l.content_type;
		if t > 1 && t <= 4 {
			return true;
		}

		let path = &self.path_list[l.path as usize];
		if path.ends_with(".js") || path.ends_with(".css") || path.ends_with(".ico") || path.ends_with(".svg") || path.ends_with(".woff") || path.ends_with(".woff2") || path.ends_with(".ttf") || path.ends_with(".eot") || path.ends_with(".otf") || path.ends_with(".feed") {
			return true;
		}

		false
	}

	pub fn is_probably_meaningless(&self, l: &LogLine) -> bool {
		let t = l.content_type;
		if t > 1 && t <= 10 {
			return true;
		}

		let path = &self.path_list[l.path as usize];
		if path.ends_with(".png") || path.ends_with(".jpg") || path.ends_with(".gif") || path.ends_with(".svg") {
			return true;
		}

		false

	}


	pub fn get_bots(&self) -> Vec<(u32, &str)> {
		let mut x: Vec<_> = self.user_agent.iter().filter(|&(x, _)| is_bot_user_agent(x)).map(|(s, &v)| (v, s.as_str())).collect();
		x.sort_unstable_by_key(|(x, _)| *x);
		x
	}
}

pub fn is_bot_user_agent(ua: &str) -> bool {
	// find the bot, crawler, spider keyword and check that it's at word boundary (next char is not a letter)
	let is_at_boundary = |word: &str| match ua.find(word) {
		Some(idx) => idx == ua.len() - word.len() || !ua.chars().nth(idx + word.len()).unwrap().is_ascii_alphanumeric(),
		None => false
	};
	is_at_boundary("bot") || is_at_boundary("Bot") || is_at_boundary("crawler") || is_at_boundary("Crawler") || is_at_boundary("spider") || is_at_boundary("Spider") || is_at_boundary("http-client") || is_at_boundary("curl") || is_at_boundary("check_http") || is_at_boundary("Miniflux") || is_at_boundary("Feedly") || is_at_boundary("okhttp") || is_at_boundary("Zapier")
}

pub struct LogLine {
	pub time: NaiveDateTime,
	pub ip: u32,
	pub http_version: u32,
	pub method: u32,
	pub domain: u32,
	pub path: u32,
	pub status_code: u32,
	pub size: u64,
	pub referer: u32,
	pub user_agent: u32,
	pub content_type: u32,
	pub compression_type: u32
}

pub fn parse_line(p: &LogParser, table: &mut GlobalTable, line: &str) -> Result<LogLine, String> {
	fn get_or_add(table: &mut HashMap<String, u32>, key: &str) -> u32 {
		if let Some(&idx) = table.get(key) {
			idx
		} else {
			let idx = table.len() as u32 + 1;
			table.insert(key.to_string(), idx);
			idx
		}
	}
	let captures = p.regex.captures(line).ok_or_else(|| "Regex didn't match")?;
	let c = |idx: usize| if p.capture_idxs[idx] == 0 { "0" } else { &captures[p.capture_idxs[idx]] };
	let time = NaiveDateTime::parse_from_str(c(0), &p.datetime_format).map_err(|e| e.to_string())?;
	let ip = get_or_add(&mut table.ip, c(1));
	let http_version = get_or_add(&mut table.http_version, c(2));
	let method = get_or_add(&mut table.method, c(3));
	let domain = get_or_add(&mut table.domain, c(4));
	let path = table.add_path(p.strip_query_string(c(5)));
	let status_code = c(6).parse().unwrap();
	let size = c(7).parse().unwrap();
	let _ = c(8);
	let referer = get_or_add(&mut table.referer, p.strip_query_string(c(9)));
	let user_agent = get_or_add(&mut table.user_agent, c(10));
	let _ = c(11);
	let _ = c(12);
	let content_type = get_or_add(&mut table.content_type, c(13));
	let compression_type = get_or_add(&mut table.compression_type, c(14));
	Ok(LogLine { time, ip, http_version, method, domain, path, status_code, size, referer, user_agent, content_type, compression_type })
}

fn skip_space(mut s: &[u8]) -> &[u8] {
	unsafe {
		while s.len() > 0 && *s.get_unchecked(0) == b' ' {
			s = s.get_unchecked(1..);
		}
		s
	}
}

fn findidx(s: &[u8], c: u8) -> Option<usize> {
	unsafe {
		for i in 0..s.len() {
			if *s.get_unchecked(i) == c {
				return Some(i);
			}
		}
		None
	}
}

fn read_date(s: &[u8]) -> (&[u8], &[u8]) {
	let midws = findidx(s, b' ').unwrap();
	let endws = findidx(&s[midws+1..], b' ').unwrap();
	(&s[0..midws+endws+1], skip_space(&s[midws+endws+2..]))
}

fn find_end_quote(s: &[u8]) -> Option<usize> {
	unsafe {
		for i in 1..s.len() {
			if *s.get_unchecked(i) == b'"' {
				if *s.get_unchecked(i-1) != b'\\' {
					return Some(i);
				}
				else {
					// count the \, if their number is odd, we can continue
					let mut cnt = 1;
					while i > cnt && *s.get_unchecked(i-cnt-1) == b'\\' {
						cnt += 1;
					}
					if (cnt % 2) == 0 {
						return Some(i);
					}
				}
			}
		}
		None
	}
}

fn read_field(s: &[u8]) -> (&[u8], &[u8]) {
unsafe {
	if s.len() > 0 && s[0] == b'"' {
		let idx = find_end_quote(s).unwrap();
		return (s.get_unchecked(1..idx), skip_space(s.get_unchecked(idx+1..)))
	}
	match findidx(s, b' ') {
		Some(idx) => (s.get_unchecked(0..idx), skip_space(s.get_unchecked(idx..))),
		None => (s, b"")
	}
}
}

fn parse_int<T: FromStr>(s: &[u8]) -> Result<T, String> {
	let str = unsafe { std::str::from_utf8_unchecked(s) };
	str.parse().ok().ok_or_else(|| format!("Failed to parse {} as {}", str, std::any::type_name::<T>()))
}

pub fn parse_line_handwritten1(p: &LogParser, table: &mut GlobalTable, line: &str) -> Result<LogLine, String> {
	unsafe fn get_or_add(table: &mut HashMap<String, u32>, key: &[u8]) -> u32 {
		let key = std::str::from_utf8_unchecked(key);
		if let Some(&idx) = table.get(key) {
			idx
		} else {
			let idx = table.len() as u32 + 1;
			table.insert(key.to_string(), idx);
			idx
		}
	}

	unsafe {
		let s = skip_space(line.as_bytes());

		let (time, s) = read_date(s);
		let (ip, s) = read_field(s);
		let (http_version, s) = read_field(s);
		let (method, s) = read_field(s);
		let (domain, s) = read_field(s);
		let (path, s) = read_field(s);
		let (status_code, s) = read_field(s);
		let (size, s) = read_field(s);
		let (_, s) = read_field(s);
		let (referer, s) = read_field(s);
		let (user_agent, s) = read_field(s);
		let (_, s) = read_field(s);
		let (_, s) = read_field(s);
		let (content_type, s) = read_field(s);
		let (compression_type, _s) = read_field(s);

		assert_ne!(domain.len(), 0);
		assert_ne!(size.len(), 0);
		assert_ne!(status_code.len(), 0);

		// log!("time = {}, ip = {}, httpv = {}, method = {}", std::str::from_utf8_unchecked(time), std::str::from_utf8_unchecked(ip), std::str::from_utf8_unchecked(http_version), std::str::from_utf8_unchecked(method));

		let time = NaiveDateTime::parse_from_str(std::str::from_utf8_unchecked(time), &p.datetime_format).map_err(|e| e.to_string())?;

		let ip = get_or_add(&mut table.ip, ip);
		let http_version = get_or_add(&mut table.http_version, http_version);
		let method = get_or_add(&mut table.method, method);
		let domain = get_or_add(&mut table.domain, domain);
		let path = table.add_path(std::str::from_utf8_unchecked(p.strip_query_string_u8(path)));
		let status_code = parse_int(status_code)?;
		let size = parse_int(size)?;
		let referer = get_or_add(&mut table.referer, p.strip_query_string_u8(referer));
		let user_agent = get_or_add(&mut table.user_agent, user_agent);
		let content_type = get_or_add(&mut table.content_type, content_type);
		let compression_type = get_or_add(&mut table.compression_type, compression_type);
		Ok(LogLine { time, ip, http_version, method, domain, path, status_code, size, referer, user_agent, content_type, compression_type })
	}
}
