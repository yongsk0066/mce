/**
 * Hyphenation integration tests.
 *
 * Validates: hyphenate(), hyphenate_text(), edge cases.
 */

import { engine } from '../setup'

const EXACT_CASES = [
  { word: 'suomalainen', expected: 'suo-ma-lai-nen' },
  { word: 'tietokone', expected: 'tie-to-ko-ne' },
  { word: 'kirjakauppa', expected: 'kir-ja-kaup-pa' },
  { word: 'jalkapallo', expected: 'jal-ka-pal-lo' },
  { word: 'aamupala', expected: 'aa-mu-pa-la' },
  { word: 'äiti', expected: 'äi-ti' },
  { word: 'öljy', expected: 'öl-jy' },
]

const CONTAINS_HYPHEN = [
  'rautatieasema',
  'presidentti',
  'eduskunta',
  'yliopisto',
  'jääkaappi',
]

describe('hyphenate() - exact matches', () => {
  it.each(EXACT_CASES)(
    'hyphenate("$word") = "$expected"',
    ({ word, expected }) => {
      expect(engine.hyphenate(word)).toBe(expected)
    },
  )
})

describe('hyphenate() - contains hyphens', () => {
  it.each(CONTAINS_HYPHEN)('hyphenate("%s") contains hyphens', (word) => {
    const result = engine.hyphenate(word)
    expect(result).toContain('-')
  })
})

describe('hyphenate() - edge cases', () => {
  it('single character is unchanged', () => {
    expect(engine.hyphenate('a')).toBe('a')
  })

  it('short word may not have hyphens', () => {
    const result = engine.hyphenate('on')
    expect(typeof result).toBe('string')
    // "on" is too short to hyphenate
  })
})

describe('hyphenate_text() - full text', () => {
  const SENTENCES = [
    'Ulosottovelallisten määrä ylitti viime vuonna ensimmäistä kertaa rajan.',
    'Kiinteistöjen ja asunto-osakkeiden myynnit kasvoivat tuntuvasti.',
  ]

  it.each(SENTENCES)(
    'hyphenates sentence and preserves structure',
    (sentence) => {
      const result = engine.hyphenate_text(sentence)
      expect(result).toContain('-')
      expect(result.length).toBeGreaterThanOrEqual(sentence.length)
    },
  )

  it('preserves spaces and punctuation in paragraph', () => {
    const paragraph = 'Suomen presidentti asuu virallisesti Helsingissä Mäntyniemen residenssissä.'
    const result = engine.hyphenate_text(paragraph)
    expect(result).toContain('-')
    expect(result).toContain(' ')
    expect(result).toContain('.')
  })
})
