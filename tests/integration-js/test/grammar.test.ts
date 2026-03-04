/**
 * Grammar checking integration tests.
 *
 * Validates: grammar_check() error detection, error structure,
 * correct sentences, edge cases.
 */

import { engine } from '../setup'

describe('grammar_check() - error detection', () => {
  it('detects REPEATED_WORD', () => {
    const errors = JSON.parse(engine.grammar_check('koira koira juoksee.'))
    const found = errors.some((e: any) => e.code === 'REPEATED_WORD')
    expect(found).toBe(true)
  })

  it('detects DOUBLE_SPACE', () => {
    const errors = JSON.parse(engine.grammar_check('koira  juoksee  nopeasti.'))
    const found = errors.some((e: any) => e.code === 'DOUBLE_SPACE')
    expect(found).toBe(true)
  })

  it('detects CAPITALIZATION_ERROR', () => {
    const errors = JSON.parse(engine.grammar_check('koira juoksee pihalla.'))
    const found = errors.some((e: any) => e.code === 'CAPITALIZATION_ERROR')
    expect(found).toBe(true)
  })

  it('detects CAPITALIZATION_ERROR for known proper nouns', () => {
    const errors = JSON.parse(engine.grammar_check('helsinki on Suomen pääkaupunki.'))
    const found = errors.some((e: any) => e.code === 'CAPITALIZATION_ERROR')
    expect(found).toBe(true)
  })

  it('detects REPEATED_WORD in different positions', () => {
    const errors = JSON.parse(engine.grammar_check('Suuri suuri talo seisoo mäellä.'))
    const found = errors.some((e: any) => e.code === 'REPEATED_WORD')
    expect(found).toBe(true)
  })

  it('detects DOUBLE_SPACE with multiple spaces', () => {
    const errors = JSON.parse(engine.grammar_check('Koira   juoksee   pihalla.'))
    const found = errors.some((e: any) => e.code === 'DOUBLE_SPACE')
    expect(found).toBe(true)
  })

  it('detects errors in trailing space before period', () => {
    const errors = JSON.parse(engine.grammar_check('Koira juoksee .'))
    expect(errors.length).toBeGreaterThan(0)
  })
})

describe('grammar_check() - correct sentences', () => {
  it('returns no errors for correct sentence', () => {
    const errors = JSON.parse(engine.grammar_check('Koira juoksee pihalla.'))
    expect(errors).toHaveLength(0)
  })

  it('returns no errors for well-formed sentence', () => {
    const errors = JSON.parse(engine.grammar_check('Suomen presidentti asuu Helsingissä.'))
    expect(errors).toHaveLength(0)
  })
})

describe('grammar_check() - real Finnish sentences', () => {
  const REAL_SENTENCES = [
    'Lappeenrannan kaupunki suunnittelee pitävänsä rakennuksen itsellään.',
    'Nykyisessä markkinatilanteessa vuokralaisen löytäminen on vaikeaa.',
    'Finnair kääntyi ulkoministeriön puoleen.',
  ]

  it.each(REAL_SENTENCES)(
    'no critical errors in: "%s"',
    (sentence) => {
      const errors = JSON.parse(engine.grammar_check(sentence))
      const critical = errors.filter((e: any) =>
        ['REPEATED_WORD', 'AGREEMENT_ERROR'].includes(e.code),
      )
      expect(critical).toHaveLength(0)
    },
  )
})

describe('grammar_check() - error structure', () => {
  it('error objects have required fields', () => {
    const errors = JSON.parse(engine.grammar_check('koira  koira juoksee.'))
    expect(errors.length).toBeGreaterThan(0)
    for (const e of errors) {
      expect(typeof e.start).toBe('number')
      expect(typeof e.end).toBe('number')
      expect(typeof e.code).toBe('string')
      expect(typeof e.message).toBe('string')
      expect(Array.isArray(e.suggestions)).toBe(true)
    }
  })
})

describe('grammar_check() - edge cases', () => {
  it('handles empty string', () => {
    const errors = JSON.parse(engine.grammar_check(''))
    expect(Array.isArray(errors)).toBe(true)
  })
})
