import svelte from 'rollup-plugin-svelte';
import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import livereload from 'rollup-plugin-livereload';
import { terser } from 'rollup-plugin-terser';
import sveltePreprocess from 'svelte-preprocess';
import typescript from '@rollup/plugin-typescript';
import replace from '@rollup/plugin-replace'
import { wasm } from '@rollup/plugin-wasm';
import copy from 'rollup-plugin-copy'

const watch = process.env.ROLLUP_WATCH
const production = false; !watch;
const halfProduction = production || !!process.env.HALF_PRODUCTION;

// generate SVG with the current commit hash
// require('child_process').spawn('bash', ['generate_commit_image.sh'], {
//     stdio: ['ignore', 'inherit', 'inherit'],
//     shell: true
// });

function serve() {
    let server;
    function toExit() {
        if (server) server.kill(0);
    }

    return {
        writeBundle() {
            if (server) return;
            server = require('child_process').spawn('yarn', ['start-sirv', '--', '--dev'], {
                stdio: ['ignore', 'inherit', 'inherit'],
                shell: true
            });

            process.on('SIGTERM', toExit);
            process.on('exit', toExit);
        }
    };
}

function plugins() {
    return [
        svelte({
            // enable run-time checks when not in production
            dev: !production,
            // we'll extract any component CSS out into
            // a separate file - better for performance
            css: css => {
                css.write('bundle.css');
            },
            preprocess: sveltePreprocess(),
        }),

        wasm(),
        resolve({
            browser: true,
            dedupe: ['svelte']
        }),
        commonjs(),
        typescript({
            sourceMap: !production,
            inlineSources: !production
        }),
        replace({
        }),


        // Watch the `public` directory and refresh the
        // browser on changes when not in production
        watch && livereload('public'),

        // If we're building for production (npm run build
        // instead of npm run dev), minify
        production && terser(),
        copy({
        targets: [
            { src: 'node_modules/logparser/*.wasm', dest: 'public/build' },
        ]})      
    ]
}

export default [{
    input: 'main.ts',
    output: {
        sourcemap: !halfProduction,
        format: 'es',
        file: 'public/build/bundle.js'
    },
    plugins: plugins(false).concat([
        // In dev mode, call `npm run start` once
        // the bundle has been generated
        watch && serve()
    ]),
    watch: {
        clearScreen: false
    }
}]
