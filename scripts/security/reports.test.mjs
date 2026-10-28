import assert from 'node:assert/strict';
import { test } from 'node:test';
import { npmResult, rustResult, secretsResult } from './reports.mjs';

const npmAudit = total => JSON.stringify({ auditReportVersion: 2, metadata: {
  vulnerabilities: { info: 0, low: 0, moderate: 0, high: total, critical: 0, total },
} });
const rustAudit = warnings => JSON.stringify({ database: { 'last-commit': 'fixture' },
  settings: { ignore: [] }, vulnerabilities: { count: 0, list: [] }, warnings });

test('an unavailable audit service or tool cannot produce a pass', () => {
  assert.throws(() => npmResult('{"error":{"code":"ENOAUDIT"}}', 1));
  assert.throws(() => npmResult('{}', 0));
  assert.throws(() => rustResult('{}', 0));
  assert.throws(() => secretsResult('', 0));
  assert.equal(npmResult(npmAudit(0), 1).status, 'error');
  assert.equal(rustResult(rustAudit({}), 1).status, 'error');
  assert.equal(secretsResult('[]', 126).status, 'error');
});

test('reported findings fail even if the process unexpectedly exits successfully', () => {
  assert.equal(npmResult(npmAudit(1), 0).status, 'failed');
  assert.equal(secretsResult('[{"RuleID":"fixture"}]', 0).status, 'failed');
  const advisory = { advisory: { id: 'fixture', informational: 'unsound' }, package: { name: 'fixture', version: '1' } };
  assert.equal(rustResult(rustAudit({ unsound: [advisory] }), 0).status, 'failed');
});

test('advisory exclusions cannot turn an incomplete Rust scan into a pass', () => {
  const report = JSON.parse(rustAudit({}));
  report.settings.ignore = ['RUSTSEC-2023-0071'];
  assert.throws(() => rustResult(JSON.stringify(report), 0), /without advisory exclusions/);
  delete report.settings;
  assert.throws(() => rustResult(JSON.stringify(report), 0), /without advisory exclusions/);
});

test('maintenance advisories stay visible in a completed scan', () => {
  const advisory = { advisory: { id: 'fixture', informational: 'unmaintained' }, package: { name: 'fixture', version: '1' } };
  const result = rustResult(rustAudit({ unmaintained: [advisory] }), 0);
  assert.equal(result.status, 'passed_with_warnings');
  assert.equal(result.warnings.unmaintained, 1);
});

test('warnings without advisory IDs stay visible instead of crashing the report', () => {
  const result = rustResult(rustAudit({ yanked: [{ kind: 'yanked', package: { name: 'fixture', version: '1' } }] }), 0);
  assert.equal(result.status, 'passed_with_warnings');
  assert.deepEqual(result.advisories[0], { id: null, package: 'fixture', version: '1', kind: 'yanked', url: null });
});

test('secret evidence omits matched values and contextual source text', () => {
  const result = secretsResult(JSON.stringify([{ RuleID: 'fixture', Secret: 'sensitive-value',
    Match: 'sensitive-source', Message: 'sensitive-commit', File: 'sensitive-filename' }]), 1);
  assert.deepEqual(result, { status: 'failed', findings: 1, rules: ['fixture'] });
  assert.doesNotMatch(JSON.stringify(result), /sensitive/);
});
