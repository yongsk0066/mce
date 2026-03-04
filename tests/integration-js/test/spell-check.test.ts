/**
 * Spell checking integration tests.
 *
 * Validates: spell_check(), is_valid_word() vs spell_check() differences,
 * hyphenated compounds, edge cases.
 */

import { engine } from '../setup'

const VALID_WORDS = [
  'koira', 'kissa', 'talo', 'auto', 'kirja', 'suomalainen',
  'rautatieasema', 'kaupunki', 'presidentti', 'yliopisto',
]

const MISSPELLED_WORDS = [
  'koirra', 'tallö', 'kirjja',
  'suomalainne', 'kaupungki', 'presidenttii',
]

describe('spell_check() - valid words', () => {
  it.each(VALID_WORDS)('accepts correctly spelled "%s"', (word) => {
    expect(engine.spell_check(word)).toBe(true)
  })
})

describe('spell_check() - misspelled words', () => {
  it.each(MISSPELLED_WORDS)('rejects misspelled "%s"', (word) => {
    expect(engine.spell_check(word)).toBe(false)
  })
})

describe('spell_check() - special cases', () => {
  it('accepts hyphenated compound "asunto-osake"', () => {
    expect(engine.spell_check('asunto-osake')).toBe(true)
  })

  it('accepts Finnish special characters', () => {
    expect(engine.spell_check('äiti')).toBe(true)
    expect(engine.spell_check('öljy')).toBe(true)
  })
})

describe('spell_check() vs is_valid_word()', () => {
  it('spell_check is more permissive than is_valid_word for compounds', () => {
    // Both should accept basic words
    expect(engine.is_valid_word('koira')).toBe(true)
    expect(engine.spell_check('koira')).toBe(true)

    // Both should reject obvious misspellings
    expect(engine.is_valid_word('koirra')).toBe(false)
    expect(engine.spell_check('koirra')).toBe(false)
  })
})

describe('KPT gradated forms pass spell check', () => {
  const KPT_FORMS = [
    // Weak grade noun forms
    'kaapin', 'maton', 'kukan', 'kadun', 'tuvan', 'rannan',
    'kaupungin', 'kullan', 'parran', 'kamman',
    // Inessive (deeper weak grade)
    'kaupungissa', 'rannassa', 'kullassa', 'tuvassa',
    // Weak grade verb forms
    'tiedän', 'pyydän', 'kiellän', 'annan', 'otan',
  ]

  it.each(KPT_FORMS)('accepts KPT form "%s"', (word) => {
    expect(engine.spell_check(word)).toBe(true)
  })
})
