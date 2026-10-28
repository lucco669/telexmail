// A missing, malformed or failed scan must never become a successful result.
export function outcome(exitCode, issues, warnings = 0) {
  if (issues > 0) return 'failed';
  if (exitCode !== 0) return 'error';
  return warnings > 0 ? 'passed_with_warnings' : 'passed';
}

export function npmResult(text, exitCode) {
  const report = JSON.parse(text);
  if (report.error || report.auditReportVersion !== 2) throw new Error('npm did not return a completed audit');
  const counts = report.metadata?.vulnerabilities;
  const severities = ['info', 'low', 'moderate', 'high', 'critical'];
  if (!counts || ![...severities, 'total'].every(k => Number.isInteger(counts[k]) && counts[k] >= 0)
    || severities.reduce((n, k) => n + counts[k], 0) !== counts.total) {
    throw new Error('npm audit counts are missing or inconsistent');
  }
  return { status: outcome(exitCode, counts.total), vulnerabilities: counts };
}

export function rustResult(text, exitCode) {
  const report = JSON.parse(text);
  const vulnerabilities = report.vulnerabilities;
  if (!Array.isArray(report.settings?.ignore) || report.settings.ignore.length !== 0) {
    throw new Error('RustSec must run without advisory exclusions');
  }
  if (!report.database?.['last-commit'] || !Array.isArray(vulnerabilities?.list)
    || vulnerabilities.count !== vulnerabilities.list.length) throw new Error('RustSec did not return a completed audit');
  const warnings = report.warnings ?? {};
  if (!Object.values(warnings).every(Array.isArray)) throw new Error('RustSec warning lists are malformed');
  const warningCount = Object.values(warnings).reduce((n, list) => n + list.length, 0);
  const issueCount = vulnerabilities.count + (warnings.unsound?.length ?? 0);
  return {
    status: outcome(exitCode, issueCount, warningCount),
    vulnerabilities: vulnerabilities.count,
    warnings: Object.fromEntries(Object.entries(warnings).map(([kind, list]) => [kind, list.length])),
    advisories: [...vulnerabilities.list, ...Object.values(warnings).flat()].map(item => ({
      id: item.advisory?.id ?? null, package: item.package.name, version: item.package.version,
      kind: item.advisory?.informational ?? item.kind ?? 'vulnerability',
      url: item.advisory?.id ? `https://rustsec.org/advisories/${item.advisory.id}.html` : null,
    })),
    database: report.database,
  };
}

export function secretsResult(text, exitCode) {
  const findings = JSON.parse(text);
  if (!Array.isArray(findings)) throw new Error('Gitleaks did not return a completed scan');
  return {
    status: outcome(exitCode, findings.length),
    findings: findings.length,
    // Never copy matched text, source lines, commit messages or credentials into evidence.
    rules: [...new Set(findings.map(f => f.RuleID))],
  };
}
