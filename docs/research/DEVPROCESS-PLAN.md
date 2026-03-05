# MCE 개발 프로세스 개선 계획

> 생성: 2026-03-06 | 상태: **Ready** | 관련: CI/CD, DX, Security, Testing
> 모든 결정 완료. 전체 WP 병렬 실행 준비됨.

## 요약

7개 전문 에이전트(보안, CI/CD, 웹인프라, DX, 레퍼런스, 버전/테스트, 거버넌스)가
병렬 조사 후 통합한 개발 프로세스 개선 계획. 총 8개 Work Package, ~16시간 작업.

## 현재 성숙도: 3.8 / 5.0

| 카테고리 | 점수 | 핵심 갭 |
|---------|:----:|--------|
| CI/CD 자동화 | 4.0 | 워크플로우 중복, concurrency 미설정 |
| 릴리스 프로세스 | 3.5 | 버전 검증 없음 |
| 코드 품질 게이트 | 4.5 | fmt+clippy+audit+accuracy |
| 성능 게이트 | **5.0** | 업계 최고 수준 |
| 테스트 인프라 | 4.0 | fuzz/proptest 없음, #[ignore] 131개 |
| 보안 | 3.5 | cargo-deny 없음, script injection |
| 버전 관리 | 3.0 | 크레이트 버전 비동기화 (0.1.0 vs 0.3.3) |
| 문서 | 3.5 | rustdoc 미배포 |
| 브랜치 관리 | 2.5 | branch protection 없음 |
| 개발자 경험 (DX) | 3.0 | task runner 없음 |

## 레퍼런스 프로젝트에서 채택한 패턴

| 패턴 | 출처 | 적용 WP |
|------|------|---------|
| `done` job (단일 required check) | swc | WP-3 |
| `cancel-in-progress: PR only` | swc | WP-3 |
| `timeout-minutes` 전체 적용 | serde | WP-3 |
| `permissions: contents: read` | ripgrep, serde, wasm-bindgen | WP-0 |
| `justfile` task runner | wasm-bindgen | WP-4 |
| Tag ↔ Cargo.toml 버전 검증 | ripgrep | WP-3 |
| `crate-ci/typos` 자동 검사 | dioxus | WP-7 |
| Draft PR skip | dioxus | WP-3 |
| Fuzz target 컴파일 체크 | ripgrep | WP-6 |

---

## Work Package 구조

```
WP-0 [보안 긴급] ──────────────────────────────→ (즉시, 독립)
WP-1 [거버넌스/정리] ─────────────────────────→ (즉시, 독립)
WP-2 [버전 통합] ─────────────────────────────→ (즉시, 독립)
WP-3 [CI/CD 재설계] ──── depends on WP-2 ────→
WP-4 [DX: justfile + CONTRIBUTING] ── after WP-3 →
WP-5 [Docs/WASM 서빙 정리] ──────────────────→ (독립)
WP-6 [테스트 인프라] ─────────────────────────→ (독립)
WP-7 [품질 강화] ────── depends on WP-3, WP-6 →
```

---

## WP-0: 보안 긴급 조치 (30분)

### 0-1. npm 토큰 (SKIP — 노출 이력 없음 확인됨)
- `.env`는 git에 커밋된 적 없음 (git log --all -p -- .env 확인)
- 사용자 판단: 현재 유지

### 0-2. auto-tag.yml Script Injection 수정 (**CRITICAL**)
- `${{ github.event.pull_request.head.ref }}`를 env 변수로 격리
- semver regex 검증 추가
```yaml
- name: Extract and validate version
  env:
    BRANCH: ${{ github.event.pull_request.head.ref }}
  run: |
    VERSION="${BRANCH#release/}"
    if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
      echo "::error::Invalid version: $VERSION"
      exit 1
    fi
    echo "tag=$VERSION" >> "$GITHUB_OUTPUT"
```

### 0-3. 워크플로우 최소 권한
- `ci.yml`, `perf.yml` 최상위에 `permissions: contents: read` 추가

---

## WP-1: 거버넌스 & 레포 정리 (30분)

### 1-1. GitHub 설정 변경
- "Automatically delete head branches" 활성화
- "Allow rebase merge" 비활성화
- Merge commit 유지 (결정됨)
- Wiki 비활성화, Auto-merge 활성화

### 1-2. Branch Protection on `main`
- Required checks: `done` job (WP-3에서 생성)
- Block force pushes
- Require conversation resolution
- Allow admin bypass

### 1-3. `.github/CODEOWNERS`
```
* @yongsk0066
```

### 1-4. 브랜치 정리 (DEFERRED)
- 사용자 판단: 작업중 브랜치가 있어 현재 정리하지 않음
- 향후 작업 완료 후 정리 예정

---

## WP-2: 버전 관리 통합 (45분)

### 2-1. Workspace 버전 통합
Root `Cargo.toml`:
```toml
[workspace.package]
version = "0.3.3"
```
11개 crate: `version.workspace = true`

### 2-2. Stale 빌드 아티팩트 .gitignore
```
crates/mce-wasm/pkg/
demo/pkg/
```

### 2-3. scripts/bump-version.sh
- workspace version 업데이트
- CHANGELOG.md 자동 편집
- Cargo.lock 재생성 + 검증

---

## WP-3: CI/CD 재설계 (2시간)

### 3-1. 통합 ci.yml (ci + release-candidate 병합)
```yaml
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

permissions:
  contents: read

jobs:
  check:           # fmt + clippy + test (15min)
  audit:           # cargo audit (5min)
  wasm:            # build + size + artifact (10min)
  integration-js:  # JS tests, PR only (10min)
  dict-tests:      # 131 #[ignore] tests (10min)
  docs:            # cargo doc -D warnings (5min)
  done:            # needs: all — 단일 required check
```

### 3-2. release-candidate.yml 삭제

### 3-3. perf.yml 업데이트
- concurrency + timeout-minutes
- Draft PR skip

### 3-4. release.yml 슬림화
- CI 재검증 제거
- validate job: tag ↔ Cargo.toml 검증
- .npmrc 파일 쓰기 → NODE_AUTH_TOKEN env만

### 3-5. auto-tag.yml 강화 (WP-0-2 포함)
- env 변수 격리 + semver 검증
- Cargo.toml 매칭
- release 브랜치 자동 삭제

### 3-6. pr-title.yml 신규
- amannn/action-semantic-pull-request

---

## WP-4: DX — justfile + CONTRIBUTING (2.5시간)

### 4-1. justfile (~140줄)
주요 recipes: check, test, test-all, wasm, wasm-size, js-test,
eval, bench, demo, doc, release-check, version, clean, stats

### 4-2. CONTRIBUTING.md 전면 리라이트 (68→~320줄)
### 4-3. PR 템플릿 강화
### 4-4. Pre-commit hooks: **lefthook v2.1.2** (결정됨)

**선정 근거**: Node.js 불필요(Go 바이너리), 병렬 실행 내장, glob 필터링,
piped 순차 모드, lefthook-local.yml 개인 오버라이드. Rust+WASM 프로젝트에 최적.

**비교 (조사 완료)**:

| | lefthook | husky | pre-commit | cargo-husky |
|---|---------|-------|-----------|-------------|
| 런타임 의존성 | 없음 (Go) | Node.js | Python | Rust |
| 병렬 실행 | 내장 | 없음 | 제한적 | 없음 |
| glob 필터링 | 내장 | 수동 | 설정 | 없음 |
| 유지보수 | 활발 (2026.02) | ~1년 전 | 활발 | 2022 중단 |

**설치**: `brew install lefthook && lefthook install`

**lefthook.yml 설정**:
```yaml
min_version: 2.1.0

output:
  - meta
  - summary
  - failure

pre-commit:
  parallel: true
  jobs:
    - name: rust-fmt-check
      glob: "*.rs"
      run: cargo fmt -- --check
      fail_text: "Run 'cargo fmt' to fix formatting"
    - name: rust-clippy
      glob: "*.rs"
      run: cargo clippy --workspace --all-targets -- -D warnings
      fail_text: "Fix clippy warnings before committing"

pre-push:
  piped: true
  jobs:
    - name: rust-fmt-check
      priority: 1
      glob: "*.rs"
      run: cargo fmt -- --check
    - name: rust-clippy
      priority: 2
      glob: "*.rs"
      run: cargo clippy --workspace --all-targets -- -D warnings
    - name: rust-test
      priority: 3
      run: cargo test --workspace

commit-msg:
  jobs:
    - name: conventional-commit
      run: |
        MSG=$(head -1 {1})
        if ! echo "$MSG" | grep -qE '^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\(.+\))?(!)?: .{1,}$'; then
          echo "ERROR: Commit message must follow Conventional Commits"
          echo "  <type>(<scope>): <description>"
          echo "  Your message: $MSG"
          exit 1
        fi
```

**WIP 커밋 우회**: `LEFTHOOK=0 git commit -m "WIP"` 또는 `--no-verify`

**.gitignore에 추가**: `lefthook-local.yml`

### 4-5. 잡다한 정리 (dictionaries/ 삭제, .editorconfig, .gitignore)

---

## WP-5: Docs/WASM 서빙 정리 (30분)

### 5-1. docs/ 바이너리를 git에서 제거
```gitignore
docs/mce_wasm_bg.wasm
docs/mce_wasm.js
docs/mor.vfst
docs/suffix_tagger.bin
docs/wordlist.txt
```

### 5-2. docs.yml 수정 — data/ 파일 전체 복사
- 현재 버그: mor.vfst, suffix_tagger.bin 미복사 → 수정

### 5-3. 로컬 데모 빌드 (justfile demo recipe로 대체)

---

## WP-6: 테스트 인프라 강화 (6시간)

### 6-1. Fuzz 테스트 (2시간)
- fuzz/ 디렉토리, 4개 타겟
- .github/workflows/fuzz.yml (on-demand)

### 6-2. FST 파서 bounds checking (1시간)

### 6-3. proptest 도입 (2시간)
- 코모나드 법칙, 자음교체 roundtrip, 토크나이저

### 6-4. 코드 커버리지 (1시간)
- cargo-llvm-cov + Codecov

---

## WP-7: 품질 강화 (3시간)

### 7-1. cargo-deny + deny.toml (45분)
### 7-2. GPL 라이선스 정리 (1시간)
### 7-3. Demo SRI 해시 (30분)
### 7-4. Dependabot auto-merge (15분)
### 7-5. Typo 검사 — crate-ci/typos (10분)
### 7-6. 스케줄 CI — 매주 월요일 (5분)

---

## 결정 사항 (2026-03-06)

- [x] PR merge 전략: **Merge commit 유지** (현재 방식)
- [x] Branch protection: **적용** (admin bypass 허용, CI `done` job required)
- [x] GPL mor.vfst: **현재 유지 + 문서 보강** (THIRD_PARTY_NOTICES + README 강조)
- [x] npm 토큰: **현재 유지** (노출 이력 없음 확인)
- [x] 브랜치 정리: **DEFERRED** (작업중 브랜치 있음)
- [x] 실행 전략: **전체 WP 병렬 실행**
- [x] Pre-commit hook: **lefthook v2.1.2** (Go 바이너리, Node 불필요, 병렬 실행)

---

## 에이전트 조사 원본 참조

7개 에이전트 조사 결과는 Claude Code session transcript에 보존됨:
1. 보안 담당: npm 토큰, script injection, unsafe 코드, 라이선스
2. CI/CD 아키텍트: 워크플로우 통합 YAML 설계
3. 웹 인프라: WASM 아티팩트 서빙 전략 (Option A 추천)
4. DX 엔지니어: justfile, CONTRIBUTING, lefthook, #[ignore] 전략
5. 레퍼런스 조사: wasm-bindgen, swc, dioxus, ripgrep, serde
6. 버전/테스트: workspace 버전 통합, fuzz, proptest, coverage
7. 거버넌스: branch protection, CODEOWNERS, merge 전략
