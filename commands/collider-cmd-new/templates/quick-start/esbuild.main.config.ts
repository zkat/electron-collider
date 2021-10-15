import { BuildOptions } from 'esbuild'
import path from 'path'

const config: BuildOptions = {
  platform: 'node',
  entryPoints: [
    path.resolve('index.html'),
    path.resolve('main.ts'),
    path.resolve('renderer.ts'),
    path.resolve('preload.ts'),
  ],
  bundle: true,
  target: 'node16.5.0', // electron version target
}

export default config