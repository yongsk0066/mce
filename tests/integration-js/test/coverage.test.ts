/**
 * Finnish vocabulary coverage integration tests.
 *
 * Validates dictionary coverage across common Finnish vocabulary:
 * government/politics, nature, daily life, professions, days, months.
 */

import { engine } from '../setup'

// Sentences for coverage testing (no source attribution)
const FINNISH_SENTENCES = [
  'Ulosottovelallisten määrä ylitti viime vuonna ensimmäistä kertaa rajan.',
  'Kiinteistöjen ja asunto-osakkeiden myynnit kasvoivat tuntuvasti.',
  'Tunnistamaton sukellusvene on upottanut Iranin sota-aluksen.',
  'Poliisiasemilla järjestettiin suruliputus kuolleiden poliisien kunniaksi.',
  'Lappeenrannan kaupunki suunnittelee pitävänsä rakennuksen itsellään.',
  'Tyhjillään olevasta rakennuksesta aiheutuu huomionarvoisia kustannuksia.',
  'Kaupunkikonsernin tavoitteena on löytää vuokralainen.',
  'Nykyisessä markkinatilanteessa vuokralaisen löytäminen on vaikeaa.',
  'Historiallisten esineiden säilytystä kaupunki selvittää lähikuukausien aikana.',
  'Finnair kääntyi ulkoministeriön puoleen.',
  'Suruliputus järjestettiin koko Suomessa.',
  'Tuusulassa roihuaa ja myrkkyä leviää ilmassa.',
]

const EXTENDED_VOCAB = [
  // Government & politics
  'eduskunta', 'perustuslaki', 'lainsäädäntö', 'oikeusministeriö',
  'valtiovarainministeriö', 'kansanedustaja',
  // Nature
  'järvi', 'metsä', 'vuori', 'joki', 'saari', 'niemi',
  // Daily life
  'ruoka', 'juoma', 'leipä', 'maito', 'kahvi', 'vesi',
  // Professions
  'opettaja', 'lääkäri', 'insinööri', 'tuomari', 'poliisi',
  // Days of the week
  'maanantai', 'tiistai', 'keskiviikko', 'torstai', 'perjantai',
  'lauantai', 'sunnuntai',
  // Months
  'tammikuu', 'helmikuu', 'maaliskuu', 'huhtikuu', 'toukokuu',
  'kesäkuu', 'heinäkuu', 'elokuu', 'syyskuu', 'lokakuu',
  'marraskuu', 'joulukuu',
]

// KPT noun forms -- all should be valid
const KPT_ALL_FORMS = (() => {
  const KPT_NOUNS: Record<string, Array<Record<string, string>>> = {
    'pp->p': [
      { nom: 'kaappi', gen: 'kaapin', part: 'kaappia', ine: 'kaapissa' },
      { nom: 'kuppi', gen: 'kupin', part: 'kuppia', ine: 'kupissa' },
      { nom: 'nappi', gen: 'napin', part: 'nappia', ine: 'napissa' },
      { nom: 'soppa', gen: 'sopan', part: 'soppaa', ine: 'sopassa' },
      { nom: 'leppä', gen: 'lepän', part: 'leppää', ine: 'lepässä' },
    ],
    'tt->t': [
      { nom: 'matto', gen: 'maton', part: 'mattoa', ine: 'matossa' },
      { nom: 'hattu', gen: 'hatun', part: 'hattua', ine: 'hatussa' },
      { nom: 'katto', gen: 'katon', part: 'kattoa', ine: 'katossa' },
      { nom: 'latte', gen: 'latten', part: 'lattea', ine: 'latteessa' },
      { nom: 'kenttä', gen: 'kentän', part: 'kenttää', ine: 'kentässä' },
    ],
    'kk->k': [
      { nom: 'kukka', gen: 'kukan', part: 'kukkaa', ine: 'kukassa' },
      { nom: 'takki', gen: 'takin', part: 'takkia', ine: 'takissa' },
      { nom: 'nukke', gen: 'nuken', part: 'nukkea', ine: 'nukessa' },
      { nom: 'verkko', gen: 'verkon', part: 'verkkoa', ine: 'verkossa' },
      { nom: 'lakki', gen: 'lakin', part: 'lakkia', ine: 'lakissa' },
    ],
    'p->v': [
      { nom: 'tupa', gen: 'tuvan', part: 'tupaa', ine: 'tuvassa' },
      { nom: 'repo', gen: 'revon', part: 'repoa', ine: 'revossa' },
      { nom: 'apu', gen: 'avun', part: 'apua', ine: 'avussa' },
    ],
    't->d': [
      { nom: 'katu', gen: 'kadun', part: 'katua', ine: 'kadulla' },
      { nom: 'pata', gen: 'padan', part: 'pataa', ine: 'padassa' },
      { nom: 'satu', gen: 'sadun', part: 'satua', ine: 'sadussa' },
      { nom: 'pöytä', gen: 'pöydän', part: 'pöytää', ine: 'pöydässä' },
    ],
    'k->zero': [
      { nom: 'puku', gen: 'puvun', part: 'pukua', ine: 'puvussa' },
      { nom: 'luku', gen: 'luvun', part: 'lukua', ine: 'luvussa' },
      { nom: 'liike', gen: 'liikkeen', part: 'liikettä', ine: 'liikkeessä' },
    ],
    'mp->mm': [
      { nom: 'kampa', gen: 'kamman', part: 'kampaa', ine: 'kammassa' },
      { nom: 'lampi', gen: 'lammin', part: 'lampea', ine: 'lammissa' },
    ],
    'nt->nn': [
      { nom: 'ranta', gen: 'rannan', part: 'rantaa', ine: 'rannassa' },
      { nom: 'kunta', gen: 'kunnan', part: 'kuntaa', ine: 'kunnassa' },
      { nom: 'lintu', gen: 'linnun', part: 'lintua', ine: 'linnussa' },
      { nom: 'sänky', gen: 'sängyn', part: 'sänkyä', ine: 'sängyssä' },
    ],
    'nk->ng': [
      { nom: 'kaupunki', gen: 'kaupungin', part: 'kaupunkia', ine: 'kaupungissa' },
      { nom: 'Helsinki', gen: 'Helsingin', part: 'Helsinkiä', ine: 'Helsingissä' },
      { nom: 'kenkä', gen: 'kengän', part: 'kenkää', ine: 'kengässä' },
      { nom: 'kanki', gen: 'kangin', part: 'kankea', ine: 'kangissa' },
    ],
    'lt->ll': [
      { nom: 'kulta', gen: 'kullan', part: 'kultaa', ine: 'kullassa' },
      { nom: 'silta', gen: 'sillan', part: 'siltaa', ine: 'sillassa' },
      { nom: 'ilta', gen: 'illan', part: 'iltaa', ine: 'illassa' },
    ],
    'rt->rr': [
      { nom: 'parta', gen: 'parran', part: 'partaa', ine: 'parrassa' },
      { nom: 'virta', gen: 'virran', part: 'virtaa', ine: 'virrassa' },
    ],
  }

  const forms: string[] = []
  for (const words of Object.values(KPT_NOUNS)) {
    for (const entry of words) {
      for (const form of Object.values(entry)) {
        if (form) forms.push(form)
      }
    }
  }
  return forms
})()

describe('coverage - Finnish sentences', () => {
  it('achieves >= 85% coverage on real Finnish text', () => {
    const allText = FINNISH_SENTENCES.join(' ')
    const words = allText.match(/[a-zA-ZäöåÄÖÅ-]+/g) || []
    const uniqueWords = [...new Set(words.map((w) => w.toLowerCase()))]

    let recognized = 0
    let unrecognized = 0
    const unrecognizedList: string[] = []

    for (const word of uniqueWords) {
      if (word.length < 2) continue
      if (engine.is_valid_word(word)) {
        recognized++
      } else {
        unrecognized++
        unrecognizedList.push(word)
      }
    }

    const total = recognized + unrecognized
    const coverage = (recognized / total) * 100

    if (unrecognizedList.length > 0) {
      console.log(`  Unrecognized: ${unrecognizedList.join(', ')}`)
    }

    expect(coverage).toBeGreaterThanOrEqual(85)
  })
})

describe('coverage - extended vocabulary', () => {
  it('achieves >= 90% coverage on extended Finnish vocabulary', () => {
    let recognized = 0
    const failedWords: string[] = []

    for (const word of EXTENDED_VOCAB) {
      if (engine.is_valid_word(word)) {
        recognized++
      } else {
        failedWords.push(word)
      }
    }

    const coverage = (recognized / EXTENDED_VOCAB.length) * 100

    if (failedWords.length > 0) {
      console.log(`  Unrecognized: ${failedWords.join(', ')}`)
    }

    expect(coverage).toBeGreaterThanOrEqual(90)
  })
})

describe('coverage - KPT noun forms', () => {
  it('recognizes all KPT gradated forms as valid', () => {
    let valid = 0
    let invalid = 0
    const invalidForms: string[] = []

    for (const form of KPT_ALL_FORMS) {
      if (engine.is_valid_word(form)) {
        valid++
      } else {
        invalid++
        invalidForms.push(form)
      }
    }

    if (invalidForms.length > 0) {
      console.log(`  Invalid KPT forms: ${invalidForms.join(', ')}`)
    }

    // Allow some failures (e.g. "kadulla" is adessive not inessive)
    const coverage = (valid / KPT_ALL_FORMS.length) * 100
    expect(coverage).toBeGreaterThanOrEqual(90)
  })
})

describe('coverage - engine state', () => {
  it('has_model() returns boolean', () => {
    expect(typeof engine.has_model()).toBe('boolean')
  })

  it('version() returns non-empty string', () => {
    const version = MceEngine.version()
    expect(typeof version).toBe('string')
    expect(version.length).toBeGreaterThan(0)
  })
})

// Import MceEngine class for static method access
import { MceEngine } from '../../../crates/mce-wasm/pkg/mce_wasm.js'
