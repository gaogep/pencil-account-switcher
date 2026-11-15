import { rmSync } from 'node:fs';
import { resolve } from 'node:path';
export default function setup() { rmSync(resolve(`visual-report/${process.platform === 'win32' ? 'windows' : 'macos'}`), { recursive: true, force: true }); }
