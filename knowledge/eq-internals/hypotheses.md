# EQ Internals — Hypotheses (Need More Data)

## H1: eqgame.exe CXWnd vtable layout matches eqmain.dll at +0x110
- **Status**: UNTESTED
- **Evidence**: eqmain.dll WndNotification confirmed at +0x110, but eqgame.exe vtable may differ
- **Test**: Call WndNotification via vtable on an eqgame.exe window (e.g., CharacterListWnd child)
- **Confirmations**: 0

## H2: CCharacterListWnd::EnterWorld() is safe to call without prior SelectCharacter()
- **Status**: UNTESTED — GPT peer review says this is dangerous
- **Evidence**: MQ2 always calls SelectCharacter(index) first
- **Test**: Call EnterWorld() without SelectCharacter() and observe behavior
- **Confirmations**: 0

## H3: ScreenMode = 3 is required before credential entry
- **Status**: UNTESTED
- **Evidence**: MQ2 checks ScreenMode before login, sets to 3
- **Test**: Check if credential entry works without ScreenMode = 3
- **Confirmations**: 0
