import measured from '@/public/benchmarks/windows-100k.json';
import { megabytes, type Release } from './release';

type BenchmarkClient = {application:string; version:string; measuredAt:string; notes:string; resources:typeof measured.resources};
export const performance: Omit<typeof measured, 'comparisons'> & {comparisons:BenchmarkClient[]; coreBuild?: {messages:number; seconds:number; batchMessages:number; includes:string; excludes:string}} = measured;
const thunderbird = performance.comparisons.find(client => client.application === 'Thunderbird');
export const ramSavedPercent = thunderbird
  ? Math.max(0, Math.round((1 - performance.resources.privateRamMiB.median / thunderbird.resources.privateRamMiB.median) * 100)) : null;
export const benchmarkDate = new Intl.DateTimeFormat('en-GB', {
  day: 'numeric', month: 'long', year: 'numeric', timeZone: 'Europe/Warsaw',
}).format(new Date(measured.measuredAt));
export const milliseconds = (value: number) => `${Math.round(value)} ms`;
export const seconds = (value: number) => `${(value / 1000).toFixed(2)} s`;
export const mib = (value: number) => `${Math.round(value)} MiB`;
export const cpuPercent = (value: number) => value < 0.01 ? '<0.01%' : `${value.toFixed(2)}%`;

export function performanceFacts(release: Release | null) {
  const installer = release?.exe;
  const search = Math.max(...Object.entries(measured.metrics).filter(([name]) => name.startsWith('search:')).map(([, result]) => result.median));
  return [
    ...(installer ? [{value:megabytes(installer.size), label:'Windows download', note:`Installer · version ${release.version}`}]: []),
    {value:seconds(measured.warmStartupMs.median), label:'Inbox ready', note:'Median repeat launch · 100,000 messages'},
    {value:milliseconds(search), label:'Search across 100,000 messages', note:'Slowest median of three full-text queries'},
  ];
}
