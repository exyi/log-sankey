import wasm from './wasm-facade'

const parserSettings = {
	pattern: [
		"(\\d+-\\d+-\\d+ \\d+:\\d+:\\d+)", // 2021-05-01 02:16:03
		'"([^"]*)"', // "81.90.168.55"
		'"([^"]*)"', //"HTTP/1.0"
		'(\\w+)', // GET
		'([\\w\\-.]+)', //ksp.mff.cuni.cz
		'"([^"]*)"', // "/sksp/2021J/pics_system/inline_menu_background_select.png"
		'(\\d+)', // 304
		'(\\d+)', // 0
		'(\\d+)', // 0
		'"([^"]*)"',// "https://ksp.mff.cuni.cz/sksp/2021J/jksp2021.css"
		'"([^"]*)"',// "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.93 Safari/537.36"
		'"([^"]*)"',// "-"
		'(\\d+)', // 11128
		'"([^"]*)"',// "-"
		'"([^"]*)"',// "-"

	].join('\\s+'),
	captures: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
	datePattern: "%Y-%m-%d %H:%M:%S",
	ignoreQueryString: true,
	maxAge: 60*60
}


export async function loadFiles(files: File[], reportProgress: (progress: number, total: number) => void) {
	console.time("wasm")
	const s = parserSettings
	const streams: ReadableStream<Uint8Array>[] = await Promise.all(files.map(f => f.stream()))
	
	const totalSize = files.map(f => f.size).reduce((a, b) => a + b, 0)
	let currentProgress = 0
	function reportCallback(bytes: number) {
		currentProgress += bytes
		reportProgress(currentProgress, totalSize)
	}
	const wasmResult = await wasm.load_logs(streams, s.pattern, s.datePattern, new Uint32Array(s.captures), s.ignoreQueryString, s.maxAge, reportCallback)
	console.timeLog("wasm", "loaded files")

	console.timeEnd("wasm")

	console.time("wasm-compute")
	const opts = new wasm.StatsOptions(60*60, 0, 300)
	let analysis = wasm.usage_stats_by_path(opts)
	console.timeEnd("wasm-compute")
	console.log(analysis)
}

export function get_graph(
	length = 8,
	maxNodes = 30,
	threshold = 3,
	mustContain = "",
	mustStartWith = ""
): TransitionGraph {
	const opts = new wasm.StatsOptions(0, threshold, maxNodes)
	return wasm.usage_transfer_graph(opts, length, mustContain, mustStartWith)
}


