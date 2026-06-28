// web/src/features/sources/importPrompt.test.ts
import { describe, it, expect } from 'vitest'
import { buildJiraImportPrompt, caseIdFromKey } from './importPrompt'

describe('caseIdFromKey', () => {
  it('lowercases and safe-slugs an issue key', () => {
    expect(caseIdFromKey('PROJ-123')).toBe('proj-123')
    expect(caseIdFromKey('  ABC_9 ')).toBe('abc-9')
  })
})

describe('buildJiraImportPrompt', () => {
  const p = buildJiraImportPrompt('PROJ-123', 'http://127.0.0.1:7878')
  it('names the issue and the derived case id', () => {
    expect(p).toContain('Jira issue PROJ-123')
    expect(p).toContain('http://127.0.0.1:7878/api/cases/proj-123')
  })
  it('maps issue fields to case fields and tags the source', () => {
    expect(p).toContain('title')
    expect(p).toContain('expected')
    expect(p).toContain('"source":"jira"')
    expect(p).toContain('"sourceRef":"PROJ-123"')
  })
})
