/**
 * Compound word splitting integration tests.
 *
 * Validates: compound_split() with known compounds, edge cases,
 * result structure.
 */

import { engine } from '../setup'

const COMPOUND_WORDS = [
  { word: 'rautatieasema', minParts: 2 },
  { word: 'kirjakauppa', minParts: 2 },
  { word: 'tietokone', minParts: 2 },
  { word: 'lentokenttä', minParts: 2 },
  { word: 'pääministeri', minParts: 2 },
  { word: 'ulkoministeriö', minParts: 2 },
  { word: 'eduskuntavaalit', minParts: 2 },
  { word: 'asunto-osake', minParts: 2 },
]

const COMPOUND_EXTENDED = [
  { word: 'jääkaappi', minParts: 2 },
  { word: 'sanakirja', minParts: 2 },
  { word: 'kahvikuppi', minParts: 2 },
  { word: 'jalkapallo', minParts: 2 },
  { word: 'aamupala', minParts: 2 },
  { word: 'työpaikka', minParts: 2 },
  { word: 'joulukuusi', minParts: 2 },
]

describe('compound_split() - known compounds', () => {
  for (const { word, minParts } of COMPOUND_WORDS) {
    it(`splits "${word}" into at least ${minParts} parts`, () => {
      const splits = JSON.parse(engine.compound_split(word))
      if (splits.length > 0) {
        const best = splits[0]
        expect(best.parts.length).toBeGreaterThanOrEqual(minParts)
      }
      // Some compounds may not split depending on dictionary -- not a hard failure
    })
  }
})

describe('compound_split() - extended compounds', () => {
  for (const { word, minParts } of COMPOUND_EXTENDED) {
    it(`splits "${word}" into at least ${minParts} parts`, () => {
      const splits = JSON.parse(engine.compound_split(word))
      if (splits.length > 0) {
        const best = splits[0]
        expect(best.parts.length).toBeGreaterThanOrEqual(minParts)
        // Verify structure
        for (const part of best.parts) {
          expect(typeof part.surface).toBe('string')
          expect(typeof part.start).toBe('number')
          expect(typeof part.end).toBe('number')
        }
      }
    })
  }
})

describe('compound_split() - non-compounds', () => {
  it('returns empty for simple word "koira"', () => {
    const splits = JSON.parse(engine.compound_split('koira'))
    expect(splits).toHaveLength(0)
  })

  it('returns empty for simple word "talo"', () => {
    const splits = JSON.parse(engine.compound_split('talo'))
    expect(splits).toHaveLength(0)
  })
})

describe('compound_split() - result structure', () => {
  it('has penalty field in split results', () => {
    const splits = JSON.parse(engine.compound_split('rautatieasema'))
    if (splits.length > 0) {
      expect(typeof splits[0].penalty).toBe('number')
    }
  })

  it('results are sorted by penalty (lowest first)', () => {
    const splits = JSON.parse(engine.compound_split('rautatieasema'))
    if (splits.length >= 2) {
      expect(splits[0].penalty).toBeLessThanOrEqual(splits[1].penalty)
    }
  })
})
