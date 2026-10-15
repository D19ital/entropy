# Эксперимент: постоянная macOS signing identity, ARM64, сборки A → B

**Только тест, не официальный релиз.** Нужен Mac Apple Silicon. Workflow:
`.github/workflows/macos-stable-signing-test.yml`, ветка `test/macos-stable-signing`
в `D19ital/entropy`. Официальные workflows, исходный runtime и production-поставка
не меняются. Нет release, тегов, Homebrew и зависимости официальной установки от
личного GitHub. Здесь GitHub Actions используется только для эксперимента.

## Что проверяем

Обе DMG содержат **Entropy.app**, исполняемый файл `Contents/MacOS/entropy`,
bundle ID **com.ergohaven.entropy**. Различаются исполняемые файлы, а не только
имена архивов. Скрипт сборки временно заменяет ровно одну константу
`const APP_TITLE: &str = "Entropy";` в `src/main.rs`:

- A: `Entropy — signing test A`;
- B: `Entropy — signing test B`.

`APP_TITLE` в текущем коде — Rust-константа, не переменная окружения. Она передаётся
в заголовок окна. После сборок исходный файл восстанавливается через EXIT trap.
Изменение существует только в CI checkout; оно не коммитится. Если механизм
константы изменился, скрипт останавливается. Для обоих вариантов сохраняются
одинаковые имя bundle, путь установки, executable и версия Cargo.

Две отдельные `cargo build --locked --release --target aarch64-apple-darwin`
выполняются **до импорта ключа**, на native ARM64 runner `macos-15`;
результат A копируется в отдельный каталог до сборки B. Скрипт проверяет ARM64,
отсутствие Rosetta и различие файлов. Секреты доступны только следующему шагу.
Упаковщик — изолированная адаптация PR74 commit
`0dbb5907f29120d59ef6533fbb067e001289cc53` к базе
`0d0c12300687f35e005a032edc23e12e42344f96`; штатный
`scripts/build_macos_app.sh` не изменён и не вызывается.

Подпись выбирается **отпечатком**, а не CN:
`50B8A2614631D0EE1705213D19D07A52C921D665`.
Ожидаемый designated requirement:

```text
designated => certificate root = H"50b8a2614631d0ee1705213d19d07a52c921d665" and identifier "com.ergohaven.entropy"
```

Для codesign задаётся эквивалент `anchor = H"…"`. Используется `--keychain`
с временным keychain, `--timestamp=none`, hardened runtime. `security
find-identity -v` не используется для допуска: он может не показать self-signed
сертификат с `CSSMERR_TP_NOT_TRUSTED`. Проверяется SHA-1 сертификата через
`find-certificate -a -Z`, после чего реальная подпись обязана пройти strict verify.
Ни `security add-trusted-cert`, ни глобальные trust settings не меняются.

Скрипт проверяет строгую подпись bundle и executable, точный requirement без
добавочных условий, равенство requirements A/B и **разные непустые CDHash**.
Упаковка идёт в `package-A` / `package-B`: B не удаляет A. DMG проверяются
`hdiutil verify`. Это не нотариализация и не обещание автоматического доверия
Gatekeeper или сохранения TCC: именно сохранение разрешений требует ручного теста.

## Подготовка оператором — первоначальная публикация ничего не собирает

1. Проверить diff и опубликовать ветку **без** `[macos-signing-test]` в сообщении
   первого коммита. Переменную `MACOS_TEST_SIGNING_READY` оставить отсутствующей
   или `0`. Job пропускается целиком, включая сборку приложений и импорт ключа.
2. После ревью workflow и отдельного подтверждения готовности добавить в Secrets
   репозитория два Actions secrets:
   - `MACOS_TEST_CERTIFICATE_P12_BASE64` — base64 существующего тестового `.p12`,
     содержащего именно указанный сертификат **и его private key**; одна строка,
     без переносов;
   - `MACOS_TEST_CERTIFICATE_PASSWORD` — пароль этого `.p12`.
   Экспорт/загрузка выполняются владельцем на доверенном устройстве через защищённый
   интерфейс GitHub/CLI. Не присылать `.p12`, пароль или private key в чат, не
   коммитить их, не помещать в issue/artifact/log. Новый сертификат не создаётся.
3. Проверить, что GitHub Actions разрешён для fork и runner `macos-15` доступен.
   Создать repository **variable**, не secret, `MACOS_TEST_SIGNING_READY=1`.
4. Только после согласования запуска сделать новый push в эту же ветку с маркером
   **`[macos-signing-test]`** в сообщении HEAD-коммита. Например, оператор может
   использовать отдельный пустой opt-in коммит. Изменение variable само запуск
   не создаёт. У workflow нет `workflow_dispatch`: не нужно добавлять его на main.
5. После теста вернуть variable в `0`/удалить. Каждый следующий push с маркером
   при READY=1 запускает новую пару; в том числе повторный запуск разрешённого run
   может использовать секреты. Не оставлять experiment постоянно вооружённым.

Три условия job: точный repository `D19ital/entropy`, READY строго `'1'` и маркер.
Push вне ветки не запускает workflow. Нет pull_request/pull_request_target trigger.
Права GITHUB_TOKEN — `contents: read`, checkout credentials не сохраняются.
Первичная публикация сама по себе не является согласием на сборку.

## Артефакт и ручной тест A → B

Скачать успешный artifact `entropy-macos-stable-signing-arm64-<run>-<attempt>`:

- `entropy-signing-test-A-arm64.dmg`;
- `entropy-signing-test-B-arm64.dmg`;
- `SHA256SUMS`, `README.ru.md`, `signature-check.txt`.

Artifact хранится 7 дней. ZIP приложения не загружается. `signature-check.txt`
содержит public fingerprint, source commit, requirement и два CDHash; секретов нет.
Перед установкой в каталоге распакованного artifact:

```
shasum -a 256 -c SHA256SUMS
```

1. Предпочтительно использовать отдельную тестовую учётную запись macOS.
   Иначе сохранить текущую официальную Entropy.app и записать её текущие разрешения,
   чтобы затем вернуть исходную установку. Закрыть все экземпляры Entropy.
2. Открыть DMG **A**, скопировать **Entropy.app** в `/Applications/Entropy.app`
   обычным Finder copy/replace. Извлечь DMG; не запускать приложение из тома DMG.
3. Self-signed сертификат не Developer ID и не нотариализован. macOS может
   заблокировать первый запуск; использовать штатное ручное разрешение открытия
   в Privacy & Security, если система это допускает. **Не отключать Gatekeeper,
   SIP и не добавлять сертификат в глобальное доверие ради теста.** Если политика
   Mac запрещает запуск, зафиксировать блокер, а не обходить её скриптом.
4. Убедиться, что окно показывает `signing test A`. При необходимости выдать
   только реально запрошенные разрешения (Input Monitoring / Accessibility),
   перезапустить и проверить нужную работу с Bluetooth/HID/Smart Input.
   Разрешения ранее установленной официальной версии не являются baseline A:
   её подпись могла быть другой. Зафиксировать состояние именно после A.
5. Полностью закрыть A. Открыть DMG **B**, заменить **тот же**
   `/Applications/Entropy.app` через Finder, извлечь DMG. Не переименовывать bundle,
   не держать A/B как две установленные копии, не запускать из Downloads/DMG.
6. Открыть B, убедиться в `signing test B`, проверить те же функции и разрешения.
   Записать, потребовалось ли повторное согласие/удаление старой записи/перезапуск,
   а также macOS version и действия. **Не делать `tccutil reset` между A/B**:
   это уничтожит проверяемое состояние. При повторном запросе не объявлять успех.
7. Сохранить только диагностические данные без ключей; закрыть тестовую версию
   и вернуть официальную копию. Возврат к другой подписи может снова потребовать
   разрешения и не является результатом теста A → B.

При необходимости обе установленная подпись и hashes доступны штатными командами:

```
codesign --verify --deep --strict --verbose=2 /Applications/Entropy.app
codesign -d -r- /Applications/Entropy.app
codesign -dvvv /Applications/Entropy.app
```

## Очистка и границы проверки

PKCS#12 импортируется только во временный keychain. Исходный search list сохраняется
до его создания и восстанавливается. EXIT/INT/TERM cleanup плюс отдельный Actions
`if: always()` удаляют `.p12`, временный пароль и keychain при обычном успехе,
ошибке или отмене. При принудительной потере runner/SIGKILL ни один shell trap не
гарантирован: остатки ограничены временным диском hosted runner, который затем
утилизируется. В upload используется явный список только public artifacts.
Секреты не передаются cargo, не печатаются; tracing отключён перед импортом.

Локальные проверки на Linux (без сборок и ключей):

```
bash scripts/test_macos_stable_signing.sh
python3 scripts/test_macos_test_packaging.py
bash -n scripts/macos_stable_signing.sh scripts/test_macos_stable_signing.sh \
  scripts/build_macos_test_app.sh scripts/build_macos_test_pair.sh scripts/sign_macos_test_pair.sh
```

Unit-тест codesign использует **моки**; тест retry использует **моки hdiutil/sleep**.
Тест очистки использует **моки security/uname** и инертные текстовые заглушки:
проверяет восстановление search list (включая пути с пробелами) и удаление заглушек.
Они не проверяют реальный Apple codesign, импорт сертификата, DMG или TCC.
На Linux реальная macOS-подпись и UI-тест **не выполнялись**. E2E-тест PR74,
генерирующий private key, намеренно не перенесён и не запускался. Реальные сборка,
импорт существующего ключа, подпись и установка A/B остаются до явного разрешения,
настройки secrets/variable и успешного Actions run. При наличии actionlint/shellcheck
можно проверить только новый workflow и эти скрипты; весь app/firmware suite не нужен.
