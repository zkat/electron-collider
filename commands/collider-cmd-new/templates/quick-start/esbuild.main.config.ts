import { BuildOptions } from 'esbuild'
import path from 'path'

const config: BuildOptions = {
  platform: 'node',
  entryPoints: [
    path.resolve('src/index.html'),
    path.resolve('src/main.ts'),
    path.resolve('src/renderer.ts'),
    path.resolve('src/preload.ts'),
  ],
  bundle: true,
  target: 'node16.5.0', // electron version target
}

export default config