<script lang="ts">

	import * as d3_sankey from 'd3-sankey'
	import * as d3 from 'd3'
	import { onDestroy, onMount } from 'svelte';
	import { get_graph } from './logbase';
import { prevent_default } from 'svelte/internal';

	let svgElement: SVGElement | null = null;
	export let data: TransitionGraph | undefined = undefined

	let layerCount = 8
	let pathNumber = 50
	let showThreshold = 77
	let mustContain = ""
	let mustStartWith = ""

	function renderSvg() {
		data = get_graph(layerCount, pathNumber, showThreshold, mustContain, mustStartWith)
		if (!svgElement || !data)
			return

		svgElement.innerHTML = ""

		// set the dimensions and margins of the graph
		var margin = {top: 10, right: 10, bottom: 10, left: 10},
			width  = window.innerWidth - margin.left - margin.right,
			height = window.innerHeight - margin.top - margin.bottom;

		// append the svg object to the body of the page
		var svg = d3.select(svgElement)
			.attr("width", width + margin.left + margin.right)
			.attr("height", height + margin.top + margin.bottom)
		.append("g")
			.attr("transform",
				"translate(" + margin.left + "," + margin.top + ")");

		// Color scale used
		var color = d3.scaleOrdinal(d3.schemeCategory10);

		// Set the sankey diagram properties
		let nodes =
			data.layers.flatMap((l, i) =>
				l.nodes
					.map((n, nodeI) => ({
						node: nodeI + 1_000_000 * i,
						name: n.path || "/index",
						value: n.session_count,
						layer: i,
						leaveValue: n.drop_count,
						medianViewTime: n.median_view_time,
					}))
					.filter(n => n.value >= showThreshold))

		console.log(data)

		const links =
			data.layers
				.slice(0, data.layers.length - 2)
				.flatMap((l, i) =>
					l.nodes.flatMap((n, nodeI) =>
						Object.entries(n.transfer_count)
							.filter(([k, v]) => v >= showThreshold)
							.map(([nextNode, count]) => ({
								sourcePath: n.path,
								source: nodeI + 1_000_000 * i,
								target: Number(nextNode) + 1_000_000 * (i + 1),
								targetPath: data!.layers[i + 1].nodes[Number(nextNode)].path,
								layerIndex: i,
								value: count
							}))
				));

		let linkedNodes = new Set(links.map(l => l.source).concat(links.map(l => l.target)))
		nodes = nodes.filter(n => linkedNodes.has(n.node))


		console.log(nodes, links)

		var leaveGradient = svg.append("defs").append("linearGradient")
			.attr("id", "leave_gradient")
			.attr("x1", "0%")
			.attr("x2", "100%")
			.attr("y1", "0%")
			.attr("y2", "0%")
		leaveGradient.append("stop")
			.attr("offset", "0%")
			.style("stop-color", "red")
			.style("stop-opacity", 1)

		leaveGradient.append("stop")
			.attr("offset", "100%")
			.style("stop-color", "white")
			.style("stop-opacity", 0)

		// Constructs a new Sankey generator with the default settings.
		var sankeyX =
			d3_sankey.sankey()
			.nodes(nodes)
			.links(links)
			.nodeWidth(60)
			.nodePadding(2)
			.extent([[1, 1], [width, height]])
			.nodeId(n => n.node)
			.iterations(32);
		var sankey = sankeyX();

		// add in the links
		var link = svg.append("g")
			.selectAll(".link")
			.data(sankey.links)
			.join("path")
			.attr("class", "link")
			.attr("fill", "none")
			.attr("stroke", (d: any) => color(d.source.name))
			.attr("stroke-opacity", 0.7)
			.style("mix-blend-mode", "multiply")
			.attr("d", d3_sankey.sankeyLinkHorizontal())
			// .style("stroke-width", "1")
			.style("stroke-width", d => Math.max(1, d.width))
			.sort((a, b) => b.dy - a.dy);

		// add in the nodes
		var node = svg.append("g")
			.selectAll(".node")
			.data(sankey.nodes)
			.enter().append("g")
			.attr("class", "node")
			.attr("transform", d => `translate(${d.x0}, ${d.y0})`)
			.call(d3.drag()
				.subject(function(d) { return d; })
				.on("start", function() { this.parentNode.appendChild(this); })
				.on("drag", dragmove));

		// add the rectangles for the nodes
		node
			.append("rect")
			.attr("height", d => d.y1 - d.y0)
			.attr("width", sankeyX.nodeWidth())
			.style("fill", function(d) { return d.color = color(d.name); })
			.style("stroke", d => d3.rgb(d.color).darker(1))
			// Add hover text
			.append("title")
			.text(d => `${d.name}\n${d.value} sessions\n~${d.medianViewTime} sec view time`);

		// drop count
		node.append("rect")
			.attr("width", sankeyX.nodeWidth() / 2)
			.attr("height", function(d) { return (d.y1 - d.y0) * d.leaveValue / d.value; })
			.attr("x", sankeyX.nodeWidth())
			.attr("y", d => (d.y1 - d.y0) * (1 - d.leaveValue / d.value))
			.style("fill", "url(#leave_gradient)")
			.append("title")
			.text(d => `${d.name}\n${d.leaveValue} (${(d.leaveValue / d.value * 100).toFixed(1)}%) left`)

		// add in the title for the nodes
		node.append("text")
			.attr("x", sankeyX.nodeWidth() / 2)
			.attr("y", function(d) { return (d.y1 - d.y0) / 2; })
			.attr("dy", ".35em")
			// .attr("text-anchor", "end")
			.attr("transform", null)
			.text(function(d) { return d.name; })
			.attr("text-anchor", "middle")
			.attr("dominant-baseline", "central");

		// the function for moving the nodes
		function dragmove(event, node) {
			d3.select(this)
			.attr("transform",
					"translate("
					+ node.x0 + ","
					+ (node.y0 = Math.max(
						0, Math.min(height - event.dy, node.y0 + event.dy))
						) + ")");
			sankeyX.update(sankey);
			link.attr("d", d3_sankey.sankeyLinkHorizontal());
		}
	}


	// onMount(() => {
	// 	renderSvg()
	// });

	onDestroy(() => {
	})

	// $: {
	// 	data
	// 	renderSvg()
	// }
</script>


<div>
	<div class="control-panel">
		<form on:submit={e => {e.preventDefault(); return false }}>
			<button on:click={_ => { renderSvg(); return false }} type="sumbit">Reload</button> |
			Layers: <input type="number" bind:value={layerCount} /> |
			Max nodes: <input type="number" bind:value={pathNumber} step="10" /> |
			Min users: <input type="number" bind:value={showThreshold} /> |
			Must contain: <input type="text" bind:value={mustContain} /> |
			Must start with: <input type="text" bind:value={mustStartWith} /> |
		</form>
	</div>
	<svg id="sankey" width="960" height="600" bind:this={svgElement}>
	</svg>
</div>
 