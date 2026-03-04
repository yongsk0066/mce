/**
 * Morphological analysis integration tests.
 *
 * Validates: analyze(), get_baseform(), POS classification,
 * morphological attributes (STRUCTURE, NUMBER, SIJAMUOTO).
 */

import { engine } from '../setup'

// --- Test data ---

const VALID_WORDS = [
  'koira', 'kissa', 'talo', 'auto', 'kirja', 'suomalainen',
  'rautatieasema', 'kaupunki', 'presidentti', 'yliopisto',
  'eduskunta', 'hallitus', 'ministeriö', 'valtio', 'kansalainen',
  'työnantaja', 'työntekijä', 'asunto', 'rakennus', 'kiinteistö',
]

const BASEFORM_PAIRS = [
  { inflected: 'kaupunki', baseform: 'kaupunki' },
  { inflected: 'kaupungin', baseform: 'kaupunki' },
  { inflected: 'kaupunkia', baseform: 'kaupunki' },
  { inflected: 'kaupungissa', baseform: 'kaupunki' },
  { inflected: 'rakennuksen', baseform: 'rakennus' },
  { inflected: 'rakennuksesta', baseform: 'rakennus' },
  { inflected: 'vuokralaisen', baseform: 'vuokralainen' },
  { inflected: 'poliisien', baseform: 'poliisi' },
  { inflected: 'kiinteistöjen', baseform: 'kiinteistö' },
  { inflected: 'kustannuksia', baseform: 'kustannus' },
  { inflected: 'suunnittelee', baseform: 'suunnitella' },
  { inflected: 'ylitti', baseform: 'ylittää' },
  { inflected: 'kasvoivat', baseform: 'kasvaa' },
  { inflected: 'järjestettiin', baseform: 'järjestää' },
  { inflected: 'löytäminen', baseform: 'löytää' },
]

const POS_EXPECTATIONS = [
  { word: 'koira', expectedClass: 'nimisana' },
  { word: 'juoksee', expectedClass: 'teonsana' },
  { word: 'nopeasti', expectedClass: 'laatusana' },
  { word: 'suunnittelee', expectedClass: 'teonsana' },
  { word: 'kaupunki', expectedClass: 'nimisana' },
  { word: 'vaikeaa', expectedClass: 'laatusana' },
]

const MORPH_DEEP_CASES = [
  { word: 'taloissa', expectKey: 'SIJAMUOTO', expectValue: 'sisaolento' },
  { word: 'koirien', expectKey: 'NUMBER', expectValue: 'plural' },
  { word: 'koiran', expectKey: 'NUMBER', expectValue: 'singular' },
  { word: 'talossa', expectKey: 'SIJAMUOTO', expectValue: 'sisaolento' },
  { word: 'talolle', expectKey: 'SIJAMUOTO', expectValue: 'ulkotulento' },
  { word: 'taloa', expectKey: 'SIJAMUOTO', expectValue: 'osanto' },
]

const VERB_BASEFORM_PAIRS = [
  { inflected: 'puhun', baseform: 'puhua' },
  { inflected: 'puhui', baseform: 'puhua' },
  { inflected: 'puhuttiin', baseform: 'puhua' },
  { inflected: 'syön', baseform: 'syödä' },
  { inflected: 'söin', baseform: 'syödä' },
  { inflected: 'tulen', baseform: 'tuli' },    // ambiguous without context
  { inflected: 'tuli', baseform: 'tuli' },      // ambiguous without context
  { inflected: 'olen', baseform: 'olla' },
  { inflected: 'oli', baseform: 'olla' },
  { inflected: 'on', baseform: 'olla' },
  { inflected: 'menee', baseform: 'mennä' },
  { inflected: 'meni', baseform: 'mennä' },
  { inflected: 'lukee', baseform: 'lukea' },
  { inflected: 'luki', baseform: 'lukea' },
]

// --- Tests ---

describe('analyze() - morphological analysis', () => {
  it.each(VALID_WORDS)('returns at least one analysis for "%s"', (word) => {
    const analyses = JSON.parse(engine.analyze(word))
    expect(Array.isArray(analyses)).toBe(true)
    expect(analyses.length).toBeGreaterThan(0)
  })

  it('includes BASEFORM and CLASS in every analysis', () => {
    for (const word of VALID_WORDS.slice(0, 8)) {
      const analyses = JSON.parse(engine.analyze(word))
      expect(analyses.length).toBeGreaterThan(0)
      const a = analyses[0]
      expect(typeof a.BASEFORM).toBe('string')
      expect(a.BASEFORM.length).toBeGreaterThan(0)
      expect(typeof a.CLASS).toBe('string')
      expect(a.CLASS.length).toBeGreaterThan(0)
    }
  })

  it('handles empty string without crashing', () => {
    const result = JSON.parse(engine.analyze(''))
    expect(Array.isArray(result)).toBe(true)
  })

  it('handles very long compound without crashing', () => {
    const result = JSON.parse(engine.analyze('rautatieasemarakennussuunnitelma'))
    expect(Array.isArray(result)).toBe(true)
  })
})

describe('is_valid_word()', () => {
  it.each(VALID_WORDS)('recognizes "%s" as valid', (word) => {
    expect(engine.is_valid_word(word)).toBe(true)
  })

  it('recognizes capitalized forms', () => {
    expect(engine.is_valid_word('Koira')).toBe(true)
    expect(engine.is_valid_word('KOIRA')).toBe(true)
  })

  it('recognizes Finnish special characters', () => {
    expect(engine.is_valid_word('äiti')).toBe(true)
    expect(engine.is_valid_word('öljy')).toBe(true)
  })

  it('does not crash on single character', () => {
    // May or may not be valid, but should not crash
    const result = engine.is_valid_word('x')
    expect(typeof result).toBe('boolean')
  })

  it('does not crash on numeric string', () => {
    const result = engine.is_valid_word('12345')
    expect(typeof result).toBe('boolean')
  })
})

describe('get_baseform() - lemma extraction', () => {
  it.each(BASEFORM_PAIRS)(
    'extracts baseform "$baseform" from "$inflected"',
    ({ inflected, baseform }) => {
      expect(engine.get_baseform(inflected)).toBe(baseform)
    },
  )
})

describe('get_baseform() - verb baseforms', () => {
  it.each(VERB_BASEFORM_PAIRS)(
    'extracts baseform "$baseform" from "$inflected"',
    ({ inflected, baseform }) => {
      expect(engine.get_baseform(inflected)).toBe(baseform)
    },
  )
})

describe('POS classification', () => {
  it.each(POS_EXPECTATIONS)(
    '"$word" has CLASS "$expectedClass"',
    ({ word, expectedClass }) => {
      const analyses = JSON.parse(engine.analyze(word))
      const classes = analyses.map((a: any) => a.CLASS)
      expect(classes).toContain(expectedClass)
    },
  )
})

describe('morphological attributes', () => {
  it.each(MORPH_DEEP_CASES)(
    '"$word" has $expectKey = "$expectValue"',
    ({ word, expectKey, expectValue }) => {
      const analyses = JSON.parse(engine.analyze(word))
      expect(analyses.length).toBeGreaterThan(0)
      expect(analyses[0][expectKey]).toBe(expectValue)
    },
  )

  it('"rautatieasema" has STRUCTURE field with = prefix', () => {
    const analyses = JSON.parse(engine.analyze('rautatieasema'))
    expect(analyses.length).toBeGreaterThan(0)
    const structure = analyses[0].STRUCTURE
    expect(typeof structure).toBe('string')
    expect(structure).toContain('=')
  })
})
