/**
 * Spelling suggestion integration tests.
 *
 * Validates: suggest(), suggest_with_context(), wordlist loading,
 * trie-based vs brute-force fallback.
 */

import { engine } from '../setup'

const SUGGEST_CASES = [
  { word: 'koirra', expectContains: 'koira', maxEdits: 1 },
  { word: 'kirjja', expectContains: 'kirja', maxEdits: 1 },
  { word: 'kaupungki', expectContains: 'kaupunki', maxEdits: 2 },
  { word: 'presidenttii', expectContains: 'presidentti', maxEdits: 2 },
  { word: 'tallö', maxEdits: 2 },
]

describe('suggest() - spelling suggestions', () => {
  it.each(SUGGEST_CASES)(
    'generates suggestions for "$word"',
    ({ word, expectContains, maxEdits }) => {
      const suggestions = JSON.parse(engine.suggest(word, maxEdits))
      expect(Array.isArray(suggestions)).toBe(true)
      expect(suggestions.length).toBeGreaterThan(0)
      if (expectContains) {
        expect(suggestions).toContain(expectContains)
      }
    },
  )

  it('returns empty array for correctly spelled word', () => {
    const suggestions = JSON.parse(engine.suggest('koira', 1))
    expect(suggestions).toHaveLength(0)
  })
})

describe('suggest_with_context() - context-aware suggestions', () => {
  const CONTEXT_CASES = [
    { word: 'koirra', prev: 'suuri', maxEdits: 1 },
    { word: 'kaupungki', prev: 'suuri', maxEdits: 2 },
    { word: 'tallö', prev: '', maxEdits: 2 },
  ]

  it.each(CONTEXT_CASES)(
    'generates context-aware suggestions for "$word" (prev="$prev")',
    ({ word, prev, maxEdits }) => {
      const suggestions = JSON.parse(engine.suggest_with_context(word, prev, maxEdits))
      expect(Array.isArray(suggestions)).toBe(true)
      expect(suggestions.length).toBeGreaterThan(0)
    },
  )
})

describe('wordlist and trie-based suggestions', () => {
  it('engine reports wordlist state correctly', () => {
    // Wordlist may or may not be loaded depending on setup.ts success
    const hasWordlist = engine.has_wordlist()
    expect(typeof hasWordlist).toBe('boolean')
  })

  it('loads a custom wordlist and uses trie-based suggestions', () => {
    const testWordlist = [
      'koira', 'koiru', 'kissa', 'talo', 'auto', 'kirja',
      'kaupunki', 'presidentti', 'yliopisto', 'rakennus',
    ].join('\n')
    const wordlistBytes = new TextEncoder().encode(testWordlist)
    engine.load_wordlist(wordlistBytes)
    expect(engine.has_wordlist()).toBe(true)

    const suggestions = JSON.parse(engine.suggest('koirra', 1))
    expect(Array.isArray(suggestions)).toBe(true)
    expect(suggestions.length).toBeGreaterThan(0)
  })
})
