/*
 * MacroQuest: The extension platform for EverQuest
 * Copyright (C) 2002-present MacroQuest Authors
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License, version 2, as published by
 * the Free Software Foundation.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 */

#pragma once

//
// eqgame.exe offsets
//

#define __ClientDate                                               20260310u
#define __ExpectedVersionDate                                     "Mar 10 2026"
#define __ExpectedVersionTime                                     "07:56:11"
#define __ActualVersionDate_x                                      0x140989148
#define __ActualVersionTime_x                                      0x140989138
#define __ActualVersionBuild_x                                     0x14091E300

// Memory Protection
#define __MemChecker0_x                                            0x1402CBC80
#define __MemChecker1_x                                            0x1405A4950
#define __MemChecker4_x                                            0x140299120
#define __EncryptPad0_x                                            0x140D5CFC0

// EverQuestInfo
#define pinstEverQuestInfo_x                                       0x140E95AE0
#define instEQZoneInfo_x                                           0x140E95CD4 // EverQuestinfo.zoneHdr

// Misc Globals
#define __do_loot_x                                                0x14022C0D0
#define __gfMaxZoomCameraDistance_x                                0x14091F5B8
#define __gfMaxCameraDistance_x                                    0x140AC9098
#define __CurrentSocial_x                                          0x140C189B0 // CActionsWnd::StopSocial
#define __BindList_x                                               0x140D54790
#define g_eqCommandStates_x                                        0x140D55830
#define __CommandList_x                                            0x140D55A60
#define __ScreenMode_x                                             0x140DDD4FC
#define __gWorld_x                                                 0x140E90998
#define __gpbCommandEvent_x                                        0x140E8E46C
#define __ServerHost_x                                             0x140E8E658
#define __Guilds_x                                                 0x140E946A0
#define __MemCheckBitmask_x                                        0x140E96023
#define __MemCheckActive_x                                         0x140E9773D
#define __MouseEventTime_x                                         0x140F0B0B8
#define DI8__MouseState_x                                          0x140F0F268
#define __heqmain_x                                                0x140F116F8
#define DI8__Mouse_x                                               0x140F11710
#define __HWnd_x                                                   0x140F11718
#define __Mouse_x                                                  0x140F11720
#define DI8__Keyboard_x                                            0x140F11740
#define __LoginName_x                                              0x140F11EAC
#define __CurrentMapLabel_x                                        0x140F259C0
#define __LabelCache_x                                             0x140F26590
#define __ChatFilterDefs_x                                         0x140A62FE0
#define Teleport_Table_Size_x                                      0x140E8E4F4
#define Teleport_Table_x                                           0x140E8E970

////
//Section 1: Vital Offsets
////
#define instCRaid_x                                                0x140E90A30
#define pinstActiveBanker_x                                        0x140E8E3A0
#define pinstActiveCorpse_x                                        0x140E8E390
#define pinstActiveGMaster_x                                       0x140E8E398
#define pinstActiveMerchant_x                                      0x140E8E388
#define pinstAltAdvManager_x                                       0x140DDE240
#define pinstCEverQuest_x                                          0x140F11758
#define pinstCamActor_x                                            0x140DDD4E8
#define pinstCDBStr_x                                              0x140DDCFF0
#define pinstCDisplay_x                                            0x140E8E450
#define pinstControlledPlayer_x                                    0x140E8E430
#define pinstCResolutionHandler_x                                  0x140F36B38
#define pinstCSidlManager_x                                        0x140F37B30
#define pinstCXWndManager_x                                        0x140F37B28
#define instDynamicZone_x                                          0x140E94560 // Part of g_dynamicZone
#define instExpeditionLeader_x                                     0x140E945AE
#define instExpeditionName_x                                       0x140E945EE
#define pinstDZMember_x                                            0x140E94678
#define pinstDZTimerInfo_x                                         0x140E94680
#define pinstEqLogin_x                                             0x140F117F0
#define instTribute_x                                              0x140D4FDC0
#define pinstDeviceInputProxy_x                                    0x140E95B3C
#define pinstEQSoundManager_x                                      0x140DDE5C0
#define pinstEQSpellStrings_x                                      0x140DC1B80
#define pinstSGraphicsEngine_x                                     0x140F36B68
#define pinstLocalPC_x                                             0x140E909A8
#define pinstLocalPlayer_x                                         0x140E8E380
#define pinstCMercenaryClientManager_x                             0x140F0C9D8
#define pinstModelPlayer_x                                         0x140E8E3B0
#define pinstRenderInterface_x                                     0x140F36B80
#define pinstSkillMgr_x                                            0x140F0E688
#define pinstSpawnManager_x                                        0x140F0CD90
#define pinstSpellManager_x                                        0x140F0E6F0
#define pinstStringTable_x                                         0x140E90A20
#define pinstSwitchManager_x                                       0x140E8DEF0
#define pinstTarget_x                                              0x140E8E428
#define pinstTaskMember_x                                          0x140D4FDB0 // Part of g_taskManager
#define pinstTradeTarget_x                                         0x140E8E3A8
#define instTributeActive_x                                        0x140D4FDE9
#define pinstViewActor_x                                           0x140DDD4E0
#define pinstWorldData_x                                           0x140E8E178
#define pinstPlayerPath_x                                          0x140F0CDB8
#define pinstTargetIndicator_x                                     0x140F0E7A0
#define EQObject_Top_x                                             0x140E8E458

////
//Section 2:  UI Related Offsets
////
#define pinstCChatWindowManager_x                                  0x140F22B20
#define pinstCContainerMgr_x                                       0x140DDD600
#define pinstCContextMenuManager_x                                 0x140F37810
#define pinstCInvSlotMgr_x                                         0x140DDD5F0
#define pinstCItemDisplayManager_x                                 0x140F25590
#define pinstCPopupWndManager_x                                    0x140F25E20
#define pinstCSpellDisplayMgr_x                                    0x140F26470
#define pinstCTaskManager_x                                        0x140C18CF0
#define pinstEQSuiteTextureLoader_x                                0x140D666E0
#define pinstItemIconCache_x                                       0x140F22E80
#define pinstLootFiltersManager_x                                  0x140DDCA38
#define pinstGFViewListener_x                                      0x140F36FE8


////
// Section 3: Miscellaneous Offsets
////
#define __allowslashcommand_x                                      0x140256530
#define __CastRay_x                                                0x14024E6B0
#define __CastRay2_x                                               0x14024E3A0
#define __compress_block_x                                         0x140559CF0
#define __ConvertItemTags_x                                        0x14024EB10
#define __CleanItemTags_x                                          0x14008C720
#define __CreateCascadeMenuItems_x                                 0x140195DF0
#define __decompress_block_x                                       0x1405A0140
#define __DoesFileExist_x                                          0x1405A89C0
#define __EQGetTime_x                                              0x1405A4F00
#define __ExecuteCmd_x                                             0x1402235B0
#define __FixHeading_x                                             0x140661520
#define __FlushDxKeyboard_x                                        0x1403506C0
#define __get_bearing_x                                            0x140258850
#define __get_melee_range_x                                        0x140258940
#define __GetAnimationCache_x                                      0x1403D5270
#define __GetGaugeValueFromEQ_x                                    0x140518EA0
#define __GetLabelFromEQ_x                                         0x14051A480
#define __GetXTargetType_x                                         0x140663A10   // ExtendedTargetList::ExtendedTargetRoleName
#define __HeadingDiff_x                                            0x1406615A0
#define __HelpPath_x                                               0x140F0AF80   // Why?
#define __NewUIINI_x                                               0x140525EE0   // Why?
#define __ProcessDeviceEvents_x                                    0x140299720
#define __ProcessGameEvents_x                                      0x14028E0F0
#define __ProcessKeyboardEvents_x                                  0x140351E10
#define __ProcessMouseEvents_x                                     0x14028F840
#define __SaveColors_x                                             0x1401A5D40
#define __STMLToText_x                                             0x1405B9160
#define __WndProc_x                                                0x14034EE60
#define CMemoryMappedFile__SetFile_x                               0x140810180
#define DrawNetStatus_x                                            0x1402DB270
#define Util__FastTime_x                                           0x1405A4410
#define __eq_delete_x                                              0x1406DC138
#define __eq_new_x                                                 0x1406DC390
#define __CopyLayout_x                                             0x1402C8070
#define __ThrottleFrameRate_x                                      0x14027D221
#define __ThrottleFrameRateEnd_x                                   0x14027D227

////
// Section 4: Function Offsets
////

// CAAwnd
#define CAAWnd__ShowAbility_x                                      0x140361860
#define CAAWnd__Update_x                                           0x140361B60
#define CAAWnd__UpdateSelected_x                                   0x140362D80

// CAdvancedLootWnd
#define CAdvancedLootWnd__DoAdvLootAction_x                        0x1400B0840
#define CAdvancedLootWnd__DoSharedAdvLootAction_x                  0x1400A9150
#define CAdvancedLootWnd__AddPlayerToList_x                        0x14086A2D0
#define CAdvancedLootWnd__UpdateMasterLooter_x                     0x1400B11A0

// AltAdvManager
#define AltAdvManager__GetCalculatedTimer_x                        0x1401B1050
#define AltAdvManager__IsAbilityReady_x                            0x1401B14F0
#define AltAdvManager__GetAAById_x                                 0x1401B08E0
#define AltAdvManager__CanTrainAbility_x                           0x1401B06E0
#define AltAdvManager__CanSeeAbility_x                             0x1401B0390

// CharacterZoneClient
#define CharacterZoneClient__BardCastBard_x                        0x1400D8390
#define CharacterZoneClient__CalcAffectChange_x                    0x1400D9250
#define CharacterZoneClient__CalcAffectChangeGeneric_x             0x1400D94A0
#define CharacterZoneClient__CanMedOnHorse_x                       0x1400ED7F0
#define CharacterZoneClient__CanUseItem_x                          0x1400EDDB0
#define CharacterZoneClient__CanUseMemorizedSpellSlot_x            0x1400D9E80
#define CharacterZoneClient__CastSpell_x                           0x1400D9F20
#define CharacterZoneClient__CharacterZoneClient_x                 0x1400E9CB0
#define CharacterZoneClient__Cur_HP_x                              0x1400EF4F0
#define CharacterZoneClient__Cur_Mana_x                            0x1400EF6B0
#define CharacterZoneClient__FindAffectSlot_x                      0x1400DD190
#define CharacterZoneClient__GetAdjustedSkill_x                    0x1400F5DD0
#define CharacterZoneClient__GetBaseSkill_x                        0x140104320  // CharacterZoneClient::Skill (inlineable)
#define CharacterZoneClient__GetCastingTimeModifier_x              0x1400DE630
#define CharacterZoneClient__GetCurrentMod_x                       0x1400ED0F0
#define CharacterZoneClient__GetCursorItemCount_x                  0x1400F7660
#define CharacterZoneClient__GetEnduranceRegen_x                   0x1400F6550
#define CharacterZoneClient__GetFirstEffectSlot_x                  0x1400DE7B0
#define CharacterZoneClient__GetFocusCastingTimeModifier_x         0x1400CDE10
#define CharacterZoneClient__GetFocusDurationMod_x                 0x1400CFCE0
#define CharacterZoneClient__GetHPRegen_x                          0x1400F6C80
#define CharacterZoneClient__GetItemCountInInventory_x             0x1400EF140
#define CharacterZoneClient__GetItemCountWorn_x                    0x1400F5E70
#define CharacterZoneClient__GetLastEffectSlot_x                   0x1400DE830
#define CharacterZoneClient__GetManaRegen_x                        0x1400F84F0
#define CharacterZoneClient__GetModCap_x                           0x1400FFAD0
#define CharacterZoneClient__GetOpenEffectSlot_x                   0x1400DEAC0
#define CharacterZoneClient__GetPCSpellAffect_x                    0x1400DEC90
#define CharacterZoneClient__GetPctModAndMin_x                     0x1400D3370
#define CharacterZoneClient__HasSkill_x                            0x1400FB610
#define CharacterZoneClient__HitBySpell_x                          0x1400DF720
#define CharacterZoneClient__IsStackBlocked_x                      0x1400E30F0
#define CharacterZoneClient__MakeMeVisible_x                       0x1400FF5B0
#define CharacterZoneClient__Max_Endurance_x                       0x1402F6020  // PcZoneClient::Max_Endurance
#define CharacterZoneClient__Max_HP_x                              0x1400FF900
#define CharacterZoneClient__Max_Mana_x                            0x1402F6240  // PcZoneclient::Max_Mana
#define CharacterZoneClient__NotifyPCAffectChange_x                0x1400FFD60
#define CharacterZoneClient__RemovePCAffectex_x                    0x1400E6760
#define CharacterZoneClient__SpellDuration_x                       0x1400E7490
#define CharacterZoneClient__TotalEffect_x                         0x1400E8880
#define CharacterZoneClient__UseSkill_x                            0x1401052A0


// CBankWnd
#define CBankWnd__WndNotification_x                                0x140382550

// CBarterWnd
#define CBarterWnd__WndNotification_x                              0x14038D330

// CBarterSearchWnd
#define CBarterSearchWnd__WndNotification_x                        0x140388FF0
#define CBarterSearchWnd__UpdateInventoryList_x                    0x14038CDA0

// CBazaarSearchWnd
#define CBazaarSearchWnd__HandleSearchResults_x                    0x14038FBA0

// CButtonWnd
#define CButtonWnd__vftable_x                                      0x140AE8B10

// CChatWindowManager
#define CChatWindowManager__GetRGBAFromIndex_x                     0x1403B2D40
#define CChatWindowManager__InitContextMenu_x                      0x1403B2ED0
#define CChatWindowManager__FreeChatWindow_x                       0x1403B1D40
#define CChatWindowManager__SetLockedActiveChatWindow_x            0x1403BB240
#define CChatWindowManager__CreateChatWindow_x                     0x1403B1780

// ChatManagerClient
#define ChatManagerClient__Instance_x                              0x14010D890

// CChatWindow
#define CChatWindow__CChatWindow_x                                 0x1403BC2B0
#define CChatWindow__Clear_x                                       0x1403BD2B0
#define CChatWindow__WndNotification_x                             0x1403BE510
#define CChatWindow__AddHistory_x                                  0x1403BCDB0

// CContextMenu
#define CContextMenu__AddMenuItem_x                                0x1405E6A70
#define CContextMenu__RemoveMenuItem_x                             0x1405E6E80
#define CContextMenu__RemoveAllMenuItems_x                         0x1405E6E50
#define CContextMenu__CheckMenuItem_x                              0x1405E6CC0
#define CContextMenu__SetMenuItem_x                                0x1405E6EA0
#define CContextMenu__AddSeparator_x                               0x1405E6BF0

// CContextMenuManager
#define CContextMenuManager__AddMenu_x                             0x1405D8800
#define CContextMenuManager__RemoveMenu_x                          0x1405D8DE0
#define CContextMenuManager__PopupMenu_x                           0x1405D8B20
#define CContextMenuManager__Flush_x                               0x1405D8880
#define CContextMenuManager__CreateDefaultMenu_x                   0x1403CCB30

// CChatService
#define CChatService__GetNumberOfFriends_x                         0x14066E310
#define CChatService__GetFriendName_x                              0x1406B5FA0

// CComboWnd
#define CComboWnd__DeleteAll_x                                     0x1405C9700
#define CComboWnd__Draw_x                                          0x1405C9800
#define CComboWnd__GetCurChoice_x                                  0x1405C9AE0  // unused
#define CComboWnd__GetListRect_x                                   0x1405C9B80
#define CComboWnd__InsertChoice_x                                  0x1405C9EB0
#define CComboWnd__SetColors_x                                     0x1405CA240
#define CComboWnd__SetChoice_x                                     0x1405CA200
#define CComboWnd__GetItemCount_x                                  0x1405C9B70
#define CComboWnd__GetCurChoiceText_x                              0x1405C9B20  // unused
#define CComboWnd__GetChoiceText_x                                 0x1405C9AB0
#define CComboWnd__InsertChoiceAtIndex_x                           0x1405C9F70

// CContainerWnd
#define CContainerWnd__HandleCombine_x                             0x1403CA820
#define CContainerWnd__SetContainer_x                              0x14042A600
#define CContainerWnd__vftable_x                                   0x1409FDEC0

// CDisplay
#define CDisplay__cameraType_x                                     0x140DDD244
#define CDisplay__ZoneMainUI_x                                     0x140601000
#define CDisplay__PreZoneMainUI_x                                  0x1401A2C50
#define CDisplay__CleanGameUI_x                                    0x140194EB0
#define CDisplay__GetClickedActor_x                                0x1401990D0
#define CDisplay__GetUserDefinedColor_x                            0x140199D40
#define CDisplay__InitCharSelectUI_x                               0x140199FF0
#define CDisplay__ReloadUI_x                                       0x1401A4F50
#define CDisplay__RestartUI_x                                      0x140207AF0
#define CDisplay__WriteTextHD2_x                                   0x1401ABF60
#define CDisplay__TrueDistance_x                                   0x1401ABC00
#define CDisplay__SetViewActor_x                                   0x1401A8470
#define CDisplay__GetFloorHeight_x                                 0x140199330
#define CDisplay__ToggleScreenshotMode_x                           0x1401AB6F0
#define CDisplay__RealRender_World_x                               0x1401A4320

// CEditWnd
#define CEditBaseWnd__SetSel_x                                     0x14060B4C0
#define CEditWnd__DrawCaret_x                                      0x1405FCCA0  // unused
#define CEditWnd__EnsureCaretVisible_x                             0x1405FD1A0
#define CEditWnd__GetCaretPt_x                                     0x1405FD440  // unused
#define CEditWnd__GetCharIndexPt_x                                 0x1405FD480
#define CEditWnd__GetDisplayString_x                               0x1405FD6E0
#define CEditWnd__GetHorzOffset_x                                  0x1405FD930
#define CEditWnd__GetLineForPrintableChar_x                        0x1405FDA90
#define CEditWnd__GetSelStartPt_x                                  0x1405FDD80  // unused
#define CEditWnd__GetSTMLSafeText_x                                0x1405FDBC0
#define CEditWnd__PointFromPrintableChar_x                         0x1405FE940
#define CEditWnd__ReplaceSelection_x                               0x1405FECF0
#define CEditWnd__SelectableCharFromPoint_x                        0x1405FF1B0
#define CEditWnd__SetEditable_x                                    0x1405FF430
#define CEditWnd__SetWindowText_x                                  0x1405FF460

// CEverQuest
#define CEverQuest__DoPercentConvert_x                             0x14027E300
#define CEverQuest__ClickedPlayer_x                                0x1402724F0
#define CEverQuest__CreateTargetIndicator_x                        0x140272DC0
#define CEverQuest__DoTellWindow_x                                 0x14010D590 // ChatManagerClient__DisplayTellText
#define CEverQuest__OutputTextToLog_x                              0x14010DE80 // ChatManagerClient__WriteTextToLogFile
#define CEverQuest__DropHeldItemOnGround_x                         0x14027FCF0
#define CEverQuest__dsp_chat_x                                     0x14010CFC0 // ChatManagerClient__DisplayChatText
#define CEverQuest__trimName_x                                     0x1402AB210
#define CEverQuest__Emote_x                                        0x1402806E0
#define CEverQuest__GetBodyTypeDesc_x                              0x1402814C0
#define CEverQuest__GetClassDesc_x                                 0x140281520
#define CEverQuest__GetClassThreeLetterCode_x                      0x1402818F0
#define CEverQuest__GetDeityDesc_x                                 0x140281BA0
#define CEverQuest__GetLangDesc_x                                  0x140281FD0
#define CEverQuest__GetRaceDesc_x                                  0x140282370
#define CEverQuest__InterpretCmd_x                                 0x140283FB0
#define CEverQuest__LeftClickedOnPlayer_x                          0x140299790
#define CEverQuest__LMouseUp_x                                     0x140285AC0
#define CEverQuest__RightClickedOnPlayer_x                         0x140296D30
#define CEverQuest__RMouseUp_x                                     0x1402903D0
#define CEverQuest__SetGameState_x                                 0x14029C200
#define CEverQuest__UPCNotificationFlush_x                         0x1402A1B90 // CEverQuest__UniversalChatProxyNotificationFlush
#define CEverQuest__IssuePetCommand_x                              0x1402856A0
#define CEverQuest__ReportSuccessfulHeal_x                         0x1402917C0
#define CEverQuest__ReportSuccessfulHit_x                          0x1402925E0

// CGaugeWnd
#define CGaugeWnd__Draw_x                                          0x1403E0410

// CGuild
#define CGuild__FindMemberByName_x                                 0x1400C75E0
#define CGuild__GetGuildName_x                                     0x1400C7620

// CHotButtonWnd
#define CHotButtonWnd__DoHotButton_x                               0x140407250

//CHotButton
#define CHotButton__SetButtonSize_x                                0x1402BD750
#define CHotButton__SetCheck_x                                     0x1402BDA20

// CInvSlotMgr
#define CInvSlotMgr__FindInvSlot_x                                 0x140421100
#define CInvSlotMgr__MoveItem_x                                    0x140421C90
#define CInvSlotMgr__SelectSlot_x                                  0x140423FC0

// CInvSLot
#define CInvSlot__HandleRButtonUp_x                                0x14041DEC0
#define CInvSlot__SliderComplete_x                                 0x14041F6F0
#define CInvSlot__GetItemBase_x                                    0x140419520
#define CInvSlot__UpdateItem_x                                     0x14041FBB0

// CInvSlotWnd
#define CInvSlotWnd__CInvSlotWnd_x                                 0x1404261D0

// CItemDisplayWnd
#define CItemDisplayWnd__SetItem_x                                 0x140447F10
#define CItemDisplayWnd__UpdateStrings_x                           0x14044ABC0
#define CItemDisplayWnd__InsertAugmentRequest_x                    0x1404447A0
#define CItemDisplayWnd__RemoveAugmentRequest_x                    0x1404460E0
#define CItemDisplayWnd__RequestConvertItem_x                      0x140447320

// CSpellDisplayWnd
#define CSpellDisplayWnd__SetSpell_x                               0x140515900
#define CSpellDisplayWnd__UpdateStrings_x                          0x140517690

// CWndDisplayManager
#define CWndDisplayManager__FindWindow_x                           0x140553050

// CItemDisplayManager
#define CItemDisplayManager__CreateWindowInstance_x                0x140440E20

// CSpellDisplayManager
#define CSpellDisplayManager__ShowSpell_x                          0x1405174D0

// CKeyRingWnd
#define CKeyRingWnd__ExecuteRightClick_x                           0x14015A880

// CLabel
#define CLabel__UpdateText_x                                       0x1404521D0

// CListWnd
#define CListWnd__CListWnd_x                                       0x1405CD080
#define CListWnd__dCListWnd_x                                      0x1405CD4C0
#define CListWnd__vftable_x                                        0x140AE6C60
#define CListWnd__AddColumn_x                                      0x1405CDB50
#define CListWnd__AddColumn1_x                                     0x1405CDBF0
#define CListWnd__AddLine_x                                        0x1405CDD50
#define CListWnd__AddString_x                                      0x1405CE2F0
#define CListWnd__CalculateFirstVisibleLine_x                      0x1405CE860
#define CListWnd__CalculateVSBRange_x                              0x1405CEB40
#define CListWnd__ClearSel_x                                       0x1405CECF0
#define CListWnd__ClearAllSel_x                                    0x1405CEC90
#define CListWnd__CloseAndUpdateEditWindow_x                       0x1405CED50
#define CListWnd__Compare_x                                        0x1405CEF10
#define CListWnd__Draw_x                                           0x1405CF3D0
#define CListWnd__DrawColumnSeparators_x                           0x1405CFF60
#define CListWnd__DrawHeader_x                                     0x1405D0020
#define CListWnd__DrawItem_x                                       0x1405D06F0
#define CListWnd__DrawLine_x                                       0x1405D1090
#define CListWnd__DrawSeparator_x                                  0x1405D1550
#define CListWnd__EnableLine_x                                     0x1405D1970
#define CListWnd__EnsureVisible_x                                  0x1405D19D0
#define CListWnd__ExtendSel_x                                      0x1405D1AD0
#define CListWnd__GetColumnMinWidth_x                              0x1405D1E60
#define CListWnd__GetColumnWidth_x                                 0x1405D1EE0
#define CListWnd__GetCurSel_x                                      0x1405D1FE0
#define CListWnd__GetItemData_x                                    0x1405D2360
#define CListWnd__GetItemHeight_x                                  0x1405D23A0
#define CListWnd__GetItemRect_x                                    0x1405D25D0
#define CListWnd__GetItemText_x                                    0x1405D2870
#define CListWnd__GetSelList_x                                     0x1405D2AD0
#define CListWnd__GetSeparatorRect_x                               0x1405D2CC0
#define CListWnd__InsertLine_x                                     0x1405D4120
#define CListWnd__RemoveLine_x                                     0x1405D4790
#define CListWnd__SetColors_x                                      0x1405D4B50
#define CListWnd__SetColumnJustification_x                         0x1405D4B70
#define CListWnd__SetColumnLabel_x                                 0x1405D4BF0
#define CListWnd__SetColumnWidth_x                                 0x1405D4D90
#define CListWnd__SetCurSel_x                                      0x1405D4E60
#define CListWnd__SetItemColor_x                                   0x1405D50D0
#define CListWnd__SetItemData_x                                    0x1405D5170
#define CListWnd__SetItemText_x                                    0x1405D53B0
#define CListWnd__Sort_x                                           0x1405D57C0
#define CListWnd__ToggleSel_x                                      0x1405D5930
#define CListWnd__SetColumnsSizable_x                              0x1405D4DE0
#define CListWnd__SetItemWnd_x                                     0x1405D54E0
#define CListWnd__GetItemWnd_x                                     0x1405D2A60
#define CListWnd__SetItemIcon_x                                    0x1405D51B0
#define CListWnd__CalculateCustomWindowPositions_x                 0x1405CE630
#define CListWnd__SetVScrollPos_x                                  0x1405D5720

// CMapViewWnd
#define CMapViewWnd__CMapViewWnd_x                                 0x140465B60

// MapViewMap
#define MapViewMap__vftable_x                                      0x140A4B850
#define MapViewMap__Clear_x                                        0x140467820
#define MapViewMap__SetZoom_x                                      0x14046DF00
#define MapViewMap__HandleLButtonDown_x                            0x14046AA00

// CMerchantWnd
#define CMerchantWnd__DisplayBuyOrSellPrice_x                      0x14048FF80  // CMerchantWnd__PurchasePageHandler__DisplayBuyOrSellPrice
#define CMerchantWnd__PurchasePageHandler__RequestGetItem_x        0x1404968E0
#define CMerchantWnd__PurchasePageHandler__RequestPutItem_x        0x1404970C0
#define CMerchantWnd__MerchantPageHandler__SelectBuySellSlot_x     0x14049AA00
#define CMerchantWnd__SelectBuySellSlot_x                          0x140499D00
#define CMerchantWnd__PurchasePageHandler__UpdateList_x            0x14049EE90

// CPacketScrambler
#define CPacketScrambler__ntoh_x                                   0x140691380
#define CPacketScrambler__hton_x                                   0x140691370

// CSidlManagerBase
#define CSidlManagerBase__FindButtonDrawTemplate_x                 0x1405F6580
#define CSidlManagerBase__FindAnimation1_x                         0x1405F64C0
#define CSidlManagerBase__FindScreenPieceTemplate_x                0x1405F6BC0
#define CSidlManagerBase__FindScreenPieceTemplate1_x               0x1405F6960
#define CSidlManagerBase__CreateXWndFromTemplate_x                 0x1405F58C0
#define CSidlManagerBase__CreateXWndFromTemplate1_x                0x1405F5850
#define CSidlManagerBase__CreateXWnd_x                             0x1405F4E90

// CSidlManager
#define CSidlManager__CreateHotButtonWnd_x                         0x1405087A0
#define CSidlManager__CreateXWnd_x                                 0x140508960

// CSidlScreenWnd
#define CSidlScreenWnd__CalculateHSBRange_x                        0x1405BAD60
#define CSidlScreenWnd__CalculateVSBRange_x                        0x1405BAC50
#define CSidlScreenWnd__ConvertToRes_x                             0x140605800 // LayoutTools::ConvertToRes
#define CSidlScreenWnd__CreateChildrenFromSidl_x                   0x1405BAEE0
#define CSidlScreenWnd__CSidlScreenWnd1_x                          0x1405BA390
#define CSidlScreenWnd__CSidlScreenWnd2_x                          0x1405BA460
#define CSidlScreenWnd__dCSidlScreenWnd_x                          0x1405BA670
#define CSidlScreenWnd__DrawSidlPiece_x                            0x1405BB5D0
#define CSidlScreenWnd__EnableIniStorage_x                         0x1405BB800
#define CSidlScreenWnd__GetChildItem_x                             0x1405BB9A0
#define CSidlScreenWnd__GetSidlPiece_x                             0x1405BBBF0
#define CSidlScreenWnd__HandleLButtonUp_x                          0x1405C5C50 // CXWnd::HandleLButtonUp
#define CSidlScreenWnd__Init1_x                                    0x1405BBF60
#define CSidlScreenWnd__LoadIniInfo_x                              0x1405BC2D0
#define CSidlScreenWnd__LoadIniListWnd_x                           0x1405BCBA0
#define CSidlScreenWnd__LoadSidlScreen_x                           0x1405BD5A0
#define CSidlScreenWnd__m_layoutCopy_x                             0x140F36E88
#define CSidlScreenWnd__StoreIniInfo_x                             0x1405BDC20
#define CSidlScreenWnd__StoreIniVis_x                              0x1405BE410
#define CSidlScreenWnd__vftable_x                                  0x140AE5EE8
#define CSidlScreenWnd__WndNotification_x                          0x1405BE460

// CSkillMgr   a.k.a. SkillManager
#define CSkillMgr__IsAvailable_x                                   0x140339700 // SkillManager::CanUseSkill
#define CSkillMgr__GetSkillCap_x                                   0x140339A20 // SkillManager::GetSkillLimit
#define CSkillMgr__GetNameToken_x                                  0x140339950
#define CSkillMgr__IsActivatedSkill_x                              0x140339DE0
#define CSkillMgr__IsCombatSkill_x                                 0x140339E20
#define CSkillMgr__GetSkillTimerDuration_x                         0x140339D50
#define CSkillMgr__GetSkillLastUsed_x                              0x1403399A0

// CSliderWnd
#define CSliderWnd__GetValue_x                                     0x1405D6290
#define CSliderWnd__SetValue_x                                     0x1405D6AA0
#define CSliderWnd__SetNumTicks_x                                  0x1405D6920

// CSpellBookWnd
#define CSpellBookWnd__MemorizeSet_x                               0x14050EFE0

// CStmlWnd
#define CStmlWnd__AppendSTML_x                                     0x1405DB260
#define CStmlWnd__CalculateHSBRange_x                              0x1405C5600
#define CStmlWnd__CalculateVSBRange_x                              0x1405DC180
#define CStmlWnd__FastForwardToEndOfTag_x                          0x1405DCC20
#define CStmlWnd__ForceParseNow_x                                  0x1405DCCC0
#define CStmlWnd__GetVisibleText_x                                 0x1405DD390
#define CStmlWnd__MakeStmlColorTag_x                               0x1405DF120
#define CStmlWnd__MakeWndNotificationTag_x                         0x1405DF160
#define CStmlWnd__SetSTMLText_x                                    0x1405E61F0
#define CStmlWnd__StripFirstSTMLLines_x                            0x1405E63A0
#define CStmlWnd__UpdateHistoryString_x                            0x1405E6740

// CTabWnd
#define CTabWnd__Draw_x                                            0x1405D6E70
#define CTabWnd__DrawCurrentPage_x                                 0x1405D6F10
#define CTabWnd__DrawTab_x                                         0x1405D72E0
#define CTabWnd__GetTabRect_x                                      0x1405D7970
#define CTabWnd__InsertPage_x                                      0x1405D7C40
#define CTabWnd__RemovePage_x                                      0x1405D7ED0
#define CTabWnd__SetPage_x                                         0x1405D8030
#define CTabWnd__UpdatePage_x                                      0x1405D8330

// CPageWnd
#define CPageWnd__FlashTab_x                                       0x1405FF8B0
#define CPageWnd__SetTabText_x                                     0x1405FF920

// CTextOverlay
#define CTextOverlay__DisplayText_x                                0x1400C0AF0  // CBroadcast::BroadcastString
#define CBroadcast__Get_x                                          0x1400C0F20


// CTextureFont
#define CTextureFont__DrawWrappedText_x                            0x1405EDDE0
#define CTextureFont__DrawWrappedText1_x                           0x1405EDCE0
#define CTextureFont__DrawWrappedText2_x                           0x1405EDF10
#define CTextureFont__GetTextExtent_x                              0x1405EE2C0
#define CTextureFont__GetHeight_x                                  0x1405EE280

// CXMLDataManager
#define CXMLDataManager__GetXMLData_x                              0x140603F20

// CXMLSOMDocumentBase
#define CXMLSOMDocumentBase__XMLRead_x                             0x1405B77E0

// CXStr
#define CXStr__gFreeLists_x                                        0x140D66050
#define CXStr__gCXStrAccess_x                                      0x140F369E8

// CXWnd
#define CXWnd__BringToTop_x                                        0x1405C0CA0
#define CXWnd__ClrFocus_x                                          0x1405C0F90
#define CXWnd__Destroy_x                                           0x1405C1110
#define CXWnd__DoAllDrawing_x                                      0x1405C1210
#define CXWnd__DrawColoredRect_x                                   0x1405C19E0
#define CXWnd__DrawTooltip_x                                       0x1405C3040
#define CXWnd__DrawTooltipAtPoint_x                                0x1405C3100
#define CXWnd__GetChildItem_x                                      0x1405C3A00
#define CXWnd__GetChildWndAt_x                                     0x1405C3AC0
#define CXWnd__GetClientRect_x                                     0x1405C3D80
#define CXWnd__GetClientClipRect_x                                 0x1405C3C30
#define CXWnd__GetRelativeRect_x                                   0x1405C5740
#define CXWnd__GetScreenClipRect_x                                 0x1405C5850
#define CXWnd__GetScreenRect_x                                     0x1405C59D0
#define CXWnd__GetTooltipRect_x                                    0x1405C5B50
#define CXWnd__IsActive_x                                          0x1402CBD60
#define CXWnd__IsDescendantOf_x                                    0x1405C6130
#define CXWnd__IsReallyVisible_x                                   0x1405C6190
#define CXWnd__IsType_x                                            0x1405C61E0
#define CXWnd__Minimize_x                                          0x1405C62E0
#define CXWnd__ProcessTransition_x                                 0x1405C71E0
#define CXWnd__Resize_x                                            0x1405C7300
#define CXWnd__Right_x                                             0x1405C73A0
#define CXWnd__SetFocus_x                                          0x1405C7760
#define CXWnd__SetFont_x                                           0x1405C77B0
#define CXWnd__SetKeyTooltip_x                                     0x1405C78C0
#define CXWnd__SetMouseOver_x                                      0x1405C7A10
#define CXWnd__SetParent_x                                         0x1405C7A90
#define CXWnd__StartFade_x                                         0x1405C7ED0
#define CXWnd__vftable_x                                           0x140AE64C0
#define CXWnd__CXWnd_x                                             0x1405BF880
#define CXWnd__dCXWnd_x                                            0x1405C01D0

// CScreenPieceTemplate
#define CScreenPieceTemplate__IsType_x                             0x14060AC50

// CXWndManager
#define CXWndManager__DestroyAllWindows_x                          0x1405E8060
#define CXWndManager__DrawCursor_x                                 0x1405E8340
#define CXWndManager__DrawWindows_x                                0x1405E85A0
#define CXWndManager__GetKeyboardFlags_x                           0x1405E8C10
#define CXWndManager__HandleKeyboardMsg_x                          0x1405E8C50
#define CXWndManager__RemoveWnd_x                                  0x1405EC470

// CDBStr
#define CDBStr__GetString_x                                        0x140193070

// ProfileManager
#define ProfileManager__GetCurrentProfile_x                        0x14069B370

// CCharacterListWnd
#define CCharacterListWnd__SelectCharacter_x                       0x1400D5D20
#define CCharacterListWnd__EnterWorld_x                            0x1400D4B20
#define CCharacterListWnd__Quit_x                                  0x1400D5D00
#define CCharacterListWnd__UpdateList_x                            0x1400D6A60

// ItemClient
#define ItemBase__CanGemFitInSlot_x                                0x1406686F0
#define ItemBase__CreateItemTagString_x                            0x140669180
#define ItemBase__GetImageNum_x                                    0x14066B860
#define ItemBase__GetItemValue_x                                   0x14066D370
#define ItemBase__IsEmpty_x                                        0x14066F090
#define ItemBase__IsKeyRingItem_x                                  0x14066F7A0
#define ItemBase__ValueSellMerchant_x                              0x140673A00
#define ItemClient__CanDrop_x                                      0x1402BFFC0
#define ItemClient__CanGoInBag_x                                   0x1402C00F0
#define ItemClient__CreateItemClient_x                             0x1402C0380
#define ItemClient__dItemClient_x                                  0x1402BFE30

// EQ_LoadingS
#define EQ_LoadingS__SetProgressBar_x                              0x1401B2A20
#define EQ_LoadingS__Array_x                                       0x140D522A0

// PcClient
#define PcBase__GetAlternateAbilityId_x                            0x14067FB20
#define PcBase__GetCombatAbility_x                                 0x140680220
#define PcBase__GetCombatAbilityTimer_x                            0x1406802C0
#define PcBase__GetItemContainedRealEstateIds_x                    0x140680A90
#define PcBase__GetNonArchivedOwnedRealEstates_x                   0x1406814F0
#define PcClient__AlertInventoryChanged_x                          0x1402E34F0
#define PcClient__GetConLevel_x                                    0x1402E3C10  // PcClient::ConsiderTarget
#define PcClient__GetMeleeSpellFromSkillIndex_x                    0x1402E6340
#define PcClient__HasLoreItem_x                                    0x1402E73C0
#define PcZoneClient__GetItemRecastTimer_x                         0x1402F3C90
#define PcZoneClient__RemoveMyAffect_x                             0x1402F7150

// EQGroundItemListManager
#define EQGroundItem__EQGroundItem_x                               0x140228050
#define EQGroundItemListManager__Add_x                             0x140228200
#define EQGroundItemListManager__Clear_x                           0x1402282A0
#define EQGroundItemListManager__Delete_x                          0x140228360
#define EQGroundItemListManager__Instance_x                        0x1402283F0

// EQPlacedItemManager
#define EQPlacedItemManager__Add_x                                 0x140263740

// CTribute
#define CTribute__GetActiveFavorCost_x                             0x14018D7B0

// PlayerClient
#define PlayerBase__CanSee_x                                       0x140653E50
#define PlayerBase__CanSee1_x                                      0x140653F30
#define PlayerBase__GetVisibilityLineSegment_x                     0x140654420
#define PlayerBase__HasProperty_x                                  0x140654600
#define PlayerBase__IsTargetable_x                                 0x1406546C0
#define PlayerClient__ChangeBoneStringSprite_x                     0x140304E60
#define PlayerClient__GetPcClient_x                                0x140307970
#define PlayerClient__PlayerClient_x                               0x1402FD180
#define PlayerClient__SetNameSpriteState_x                         0x14030B0A0
#define PlayerClient__SetNameSpriteTint_x                          0x14030C060
#define PlayerZoneClient__ChangeHeight_x                           0x14031AB80
#define PlayerZoneClient__DoAttack_x                               0x14031B890
#define PlayerZoneClient__GetLevel_x                               0x14031EFE0
#define PlayerZoneClient__IsValidTeleport_x                        0x1402664D0
#define PlayerZoneClient__LegalPlayerRace_x                        0x1401A14D0

// PlayerManagerClient
#define PlayerManagerClient__GetSpawnByID_x                        0x140314050  // ... GetPlayerById
#define PlayerManagerClient__GetSpawnByName_x                      0x140314100  // ... GetPlayerByName
#define PlayerManagerClient__GetPlayerFromPartialName_x            0x140314200
#define PlayerManagerClient__CreatePlayer_x                        0x140313B10
#define PlayerManagerBase__PrepForDestroyPlayer_x                  0x140653A10

// KeyPressHandler
#define KeypressHandler__Get_x                                     0x1402C5560
#define KeypressHandler__AttachAltKeyToEqCommand_x                 0x1402C4E70
#define KeypressHandler__AttachKeyToEqCommand_x                    0x1402C4EE0
#define KeypressHandler__ClearCommandStateArray_x                  0x1402C54F0  // KeypressHandler::Flush
#define KeypressHandler__HandleKeyDown_x                           0x1402C5680
#define KeypressHandler__HandleKeyUp_x                             0x1402C57A0
#define KeypressHandler__SaveKeymapping_x                          0x1402C5370  // FileKeymappingSave

// PlayerPointManager
#define PlayerPointManager__GetAltCurrency_x                       0x140698E80  // PlayerPointManager::GetPoints

// StringTable
#define StringTable__getString_x                                   0x140692AA0

// PcZoneClient
#define PcZoneClient__doCombatAbility_x                            0x1402ED490  // PcZoneClient::ActivateMeleeCombatAbility
#define PcZoneClient__DestroyHeldItemOrMoney_x                     0x1402F1DB0
#define PcZoneClient__GetPcSkillLimit_x                            0x1402F4D70
#define PcZoneClient__RemovePetEffect_x                            0x1402F7430
#define PcZoneClient__HasAlternateAbility_x                        0x1402F5950
#define PcZoneClient__CanEquipItem_x                               0x1402F0070
#define PcZoneClient__GetItemByID_x                                0x1402F2D00
#define PcZoneClient__RemoveBuffEffect_x                           0x1402F6FE0
#define PcZoneClient__BandolierSwap_x                              0x1402E35A0

// Doors
#define EQSwitch__UseSwitch_x                                      0x14026B060

// IconCache
#define IconCache__GetIcon_x                                       0x1403D52A0

// CContainerMgr
#define CContainerMgr__OpenContainer_x                             0x1403C9570
#define CContainerMgr__CloseContainer_x                            0x1403C8E90
#define CContainerMgr__OpenExperimentContainer_x                   0x1403C9910

// CQuantityWnd
#define CQuantityWnd__Open_x                                       0x1404CBAE0

// CHelpWnd
#define CHelpWnd__SetFile_x                                        0x1402B7550

// CLootWnd
#define CLootWnd__LootAll_x                                        0x140459700
#define CLootWnd__RequestLootSlot_x                                0x14045A750

// EQ_Spell
#define EQ_Spell__GetSpellLevelNeeded_x                            0x1401E2D60
#define EQ_Spell__SpellAffects_x                                   0x1401E4040
#define EQ_Spell__SpellAffectBase_x                                0x1401E3FA0
#define EQ_Spell__GetSpellAffectBySlot_x                           0x1400DEE20
#define EQ_Spell__GetSpellAffectByIndex_x                          0x1400DEDD0
#define EQ_Spell__IsSPAStacking_x                                  0x1401E3C70
#define EQ_Spell__IsSPAIgnoredByStacking_x                         0x1401E3820
#define EQ_Spell__IsDegeneratingLevelMod_x                         0x1401E30E0

// EQSpellStrings
#define EQSpellStrings__GetString_x                                0x1400C60E0

// CTargetWnd
#define CTargetWnd__WndNotification_x                              0x140529730
#define CTargetWnd__RefreshTargetBuffs_x                           0x140528E50
#define CTargetWnd__HandleBuffRemoveRequest_x                      0x140528650

// CTaskWnd
#define CTaskWnd__UpdateTaskTimers_x                               0x140530FD0  // CTaskWnd::UpdateTimes

// CTaskManager
#define CTaskManager__GetEntry_x                                   0x14018A240
#define CTaskManager__GetTaskStatus_x                              0x14018A350
#define CTaskManager__GetElementDescription_x                      0x140189ED0

// EqSoundManager
#define EqSoundManager__WavePlay_x                                 0x1401E1080
#define EqSoundManager__PlayScriptMp3_x                            0x1401DFEC0
#define EqSoundManager__SoundAssistPlay_x                          0x14033EB40  // SoundAsset::Play
#define EqSoundManager__WaveInstancePlay_x                         0x14033EE80  // WaveInstance::Play

// CTextureAnimation
#define CTextureAnimation__Draw_x                                  0x1405C8CA0
#define CTextureAnimation__SetCurCell_x                            0x1405C8FB0

// FreeTargetTracker
#define FreeTargetTracker__CastSpell_x                             0x1402B5740

// CharacterBase
#define CharacterBase__GetItemByGlobalIndex_x                      0x1406B3710
#define CharacterBase__GetItemByGlobalIndex1_x                     0x1406B3790
#define CharacterBase__IsExpansionFlag_x                           0x140207140

// CCastSpellWnd
#define CCastSpellWnd__ForgetMemorizedSpell_x                      0x1403A3C40
#define CCastSpellWnd__IsBardSongPlaying_x                         0x1403A46B0
#define CCastSpellWnd__RefreshSpellGemButtons_x                    0x1403A5C10

// messages
#define msg_spell_worn_off_x                                       0x14021E2D0
#define msg_new_text_x                                             0x140219AB0
#define __msgTokenTextParam_x                                      0x140210EE0
#define msgTokenText_x                                             0x140210DF0

// SpellManager
#define Spellmanager__LoadTextSpells_x                             0x1403407A0
#define SpellManager__GetSpellByGroupAndRank_x                     0x1403406A0

// CCollisionInfoTargetVisibility
#define CCollisionInfoTargetVisibility__CCollisionInfoTargetVisibility_x 0x140663DF0

// CLargeDialogWnd
#define CLargeDialogWnd__Open_x                                    0x140452950

// CCursorAttachment
#define CCursorAttachment__AttachToCursor_x                        0x1403CE3C0
#define CCursorAttachment__IsOkToActivate_x                        0x1403D1840
#define CCursorAttachment__RemoveAttachment_x                      0x1403D1AF0
// CEQSuiteTextureLoader
#define CEQSuiteTextureLoader__CreateTexture_x                     0x140602380
#define CEQSuiteTextureLoader__GetDefaultUIPath_x                  0x140602580
#define CEQSuiteTextureLoader__GetTexture_x                        0x140602590

// CFindItemWnd
#define CFindItemWnd__WndNotification_x                            0x140155100
#define CFindItemWnd__Update_x                                     0x140155AC0
#define CFindItemWnd__PickupSelectedItem_x                         0x14014E790

// LootFiltersManager
#define LootFiltersManager__AddItemLootFilter_x                    0x14015FD10
#define LootFiltersManager__GetItemFilterData_x                    0x140160840
#define LootFiltersManager__RemoveItemLootFilter_x                 0x1401610F0
#define LootFiltersManager__SetItemLootFilter_x                    0x1401613A0

// COptionsWnd
#define COptionsWnd__FillChatFilterList_x                          0x1404AD910

// CResolutionHandler
#define CResolutionHandler__GetWindowedStyle_x                     0x1403392C0
#define CResolutionHandler__UpdateResolution_x                     0x1405AD420

// CColorPickerWnd
#define CColorPickerWnd__Open_x                                    0x1403C57F0

// CDistillerInfo
#define CDistillerInfo__GetIDFromRecordNum_x                       0x1406B7B20  // Rename..?
#define CDistillerInfo__Instance_x                                 0x1406B7A10  // ItemDistillerManager__GetInstance

// CGroupWnd
#define CGroupWnd__WndNotification_x                               0x1403EFA50
#define CGroupWnd__UpdateDisplay_x                                 0x1403EEBE0

// ItemBase
#define ItemBase__IsLore_x                                         0x14066F8E0
#define ItemBase__IsLoreEquipped_x                                 0x14066F970

#define MultipleItemMoveManager__ProcessMove_x                     0x1402D9060

// EQPlacedItemManager
#define EQPlacedItemManager__Instance_x                            0x1402638A0
#define EQPlacedItemManager__GetItemByGuid_x                       0x140263830
#define EQPlacedItemManager__GetItemByRealEstateAndRealEstateItemIds_x 0x140263870

// RealEstateManagerClient
#define RealEstateManagerClient__Instance_x                        0x140333520

// FactionManagerClient
#define FactionManagerClient__Instance_x                           0x14012B3C0
#define FactionManagerClient__HandleFactionMessage_x               0x14012B7A0
#define FactionManagerClient__GetFactionStanding_x                 0x14012AB90
#define FactionManagerClient__GetMaxFaction_x                      0x14012AB90
#define FactionManagerClient__GetMinFaction_x                      0x14012AA40

// AggroMeterManagerClient
#define AggroMeterManagerClient__Instance_x                        0x1400B2EA0

// ClientSOIManager
#define ClientSOIManager__GetSingleton_x                           0x140136D70

// MercenaryAlternateAdvancementManagerClient
#define MercenaryAlternateAdvancementManagerClient__Instance_x     0x1401BB780
#define MercenaryAlternateAdvancementManagerClient__BuyAbility_x   0x1401BA920
#define CAltAbilityData__GetMercCurrentRank_x                      0x1401BB5A0  // MercenaryAlternateAdvancementManagerClient::GetMercenaryMaxOwnedGroupRank
#define CAltAbilityData__GetMercMaxRank_x                          0x1401BB520  // MercenaryAlternateAdvancementManagerClient::GetMercenaryMaxGroupRank

// CTargetManager
#define CTargetManager__Get_x                                      0x140345AB0

// RealEstateManagerClient
#define RealEstateManagerClient__Instance_x                        0x140333520

// KeyCombo
#define KeyCombo__GetTextDescription_x                             0x1405BEA50

// CUnSerializeBuffer
#define CUnSerializeBuffer__GetString_x                            0x140088E00

// ZoneGuideManagerClient
#define ZoneGuideManagerClient__Instance_x                         0x1403571F0

// AchievementManager
#define AchievementManager__Instance_x                             0x140091FB0

// FreeToPlayClient
#define FreeToPlayClient__Instance_x                               0x14069BF00
#define FreeToPlayClient__RestrictionInfo_x                        0x140AFAE70

// UdpConnection
#define UdpConnection__GetStats_x                                  0x140560110

// CGFSCreenWnd
#define CGFScreenWnd__vftable_x                                    0x1408508E8
#define CGFScreenWnd__CGFScreenWnd_x                               0x140072AB0
#define CGFScreenWnd__CGFScreenWnd1_x                              0x140072C40
#define CGFScreenWnd__dCGFScreenWnd_x                              0x140072DF0

#if __has_include("../../../../private/eqgame-private.h")
#include "../../../../private/eqgame-private.h"
#endif
