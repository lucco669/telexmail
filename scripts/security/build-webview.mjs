import {spawnSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
const config=readFileSync('.bench/security-tauri-' + process.platform + '.conf.json','utf8');
const result=spawnSync('cargo',['build','-p','telex','--locked'],{stdio:'inherit',env:{...process.env,TAURI_CONFIG:config}});
process.exit(result.status ?? 1);
