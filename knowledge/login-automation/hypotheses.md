# Login Automation — Hypotheses

## H1: Character selection via CListWnd index matching
- **Status**: UNTESTED
- **Evidence**: MQ2 uses `GetChildWindow<CListWnd>(m_currentWindow, "Character_List")` then matches character name against list items
- **Test**: Walk Character_List CListWnd items, find name match, call SelectCharacter(index)
- **Confirmations**: 0

## H2: 10s sleep is sufficient for character select screen to load
- **Status**: UNTESTED — may be too short under load or too long normally
- **Evidence**: Works in initial tests but peer review flagged as brittle
- **Test**: Measure actual transition times across multiple logins
- **Confirmations**: 0
