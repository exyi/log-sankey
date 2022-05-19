<script lang="ts">
import { loadFiles } from "./logbase";


	let fileUpload: HTMLInputElement
	let progress = 0
	let progressText = ""
	let loading = false

	function newFile(e: Event) {
	}

	function reportProgress(current: number, total: number) {
		progress = current / total * 100
		progressText = `${(current / 1024 / 1024).toFixed(1)} MiB`
	}

	async function loadThem() {
		const files = [... fileUpload.files!]

		progress = 0
		progressText = ""
		loading = true
		await loadFiles(files, reportProgress)
		loading = false
	}
</script>

<div>
	<input type="file" bind:this={fileUpload} on:change={newFile} multiple />


	<button on:click={loadThem}>Load them</button>

	{#if loading}
		<progress value={progress} max="100"> {progress}% </progress>
		<span>{progressText}</span>
	{/if}
</div>
