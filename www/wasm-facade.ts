import init, * as parserStuff from 'logparser'

await init()

export default parserStuff


declare global {

	type UsageStatRow = {
		category: string,
		count: number[],
		time: number[]
	}
	
	type UsageStats = {
		rows: UsageStatRow[],
		start_time: number,
		end_time: number,
		session_starts_only: boolean
	}

	type TransitionGraphNode = {
		path: string,
		path_id: number,
		session_count: number,
		median_view_time: number,
		drop_count: number,
		transfer_count: { [key: number]: number } //HashMap<u32, u32>,
	}
	
	type TransitionGraphLayer = {
		nodes: TransitionGraphNode[]
	}
	
	type TransitionGraph = {
		layers: TransitionGraphLayer[]
	}
}

