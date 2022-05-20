use futures::{StreamExt, Stream};

use crate::log;

pub fn bytes_to_lines<T: Stream<Item=Vec<u8>>>(bytes: T) -> impl Stream<Item=Vec<String>> {
	let mut remainder: Vec<u8> = Vec::new();
	bytes.map(move |bytes| {
		let bytelines = bytes.split(|b| *b == b'\n').collect::<Vec<_>>();
		if bytelines.len() == 1 {
			remainder.extend(bytes);
			return vec![];
		}
		assert!(bytelines.len() > 1);
		
		let lines = bytelines.iter().enumerate().take(bytelines.len() - 1).map(|(i, &line)| {
			if i == 0 {
				let mut remainder2 = vec![];
				std::mem::swap(&mut remainder2, &mut remainder);

				remainder2.extend(line);
				return String::from_utf8(remainder2).unwrap();
			}
			String::from_utf8(line.to_vec()).unwrap()
		}).collect();

		remainder.extend(bytelines[bytelines.len() - 1]);

		lines
	})
}
