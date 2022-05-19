import p from './wasm-facade'
import App from './App.svelte'

// p.greet("Test")

let app = new App({
	target: document.getElementById("svelte-root")!,
})
