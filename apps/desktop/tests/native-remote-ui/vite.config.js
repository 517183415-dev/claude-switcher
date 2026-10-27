import {mergeConfig} from 'vite';
import {fileURLToPath} from 'node:url';
import base from '../../vite.config';

export default mergeConfig(base, {resolve: {alias: [{find: '@tauri-apps/api/core',
  replacement: fileURLToPath(new URL('./tauri-shim.js', import.meta.url))}]}});
