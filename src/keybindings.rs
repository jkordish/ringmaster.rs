use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::Screen;
use crate::navigation::{FocusRegion, TransientLayer};

const SCREEN_DIGITS: [char; Screen::ALL.len()] = ['1', '2', '3', '4', '5', '6', '7', '8'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingScope {
    Always,
    Global,
    Screen(Screen),
    Region(FocusRegion),
    ScreenRegion(Screen, FocusRegion),
    Transient(TransientLayer),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindingKind {
    Standard,
    Expert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct KeyChord {
    pub code: ChordKey,
    pub modifiers: ChordModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ChordKey {
    Char(char),
    Tab,
    BackTab,
    Left,
    Right,
    Up,
    Down,
    Enter,
    Esc,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Default)]
pub struct ChordModifiers {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Debug, Clone)]
pub struct Keybinding {
    pub scope: BindingScope,
    pub kind: BindingKind,
    pub chord: KeyChord,
    pub action: Action,
    pub label: &'static str,
    pub show_in_footer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingContext {
    pub active_screen: Screen,
    pub focused_region: FocusRegion,
    pub search_open: bool,
    pub help_open: bool,
    pub ai_preflight_open: bool,
}

static KEYBINDINGS: OnceLock<Vec<Keybinding>> = OnceLock::new();

#[must_use]
pub fn resolve(event: KeyEvent, context: BindingContext) -> Option<Action> {
    let chord = KeyChord::from_key_event(event)?;
    let bindings = bindings();
    for scope in resolve_scopes(context) {
        if let Some(binding) = bindings
            .iter()
            .find(|binding| binding.scope == scope && binding.chord == chord)
        {
            return Some(binding.action.clone());
        }
    }
    None
}

#[must_use]
pub fn footer_hints(context: BindingContext, activation_available: bool) -> Vec<&'static str> {
    let active_scopes = resolve_scopes(context);
    let mut seen = BTreeSet::new();
    bindings()
        .iter()
        .filter(|binding| {
            binding.kind == BindingKind::Standard
                && binding.show_in_footer
                && active_scopes.contains(&binding.scope)
                && (activation_available || binding.action != Action::ActivateFocusedRegion)
        })
        .filter_map(|binding| {
            if seen.insert(binding.label) {
                Some(binding.label)
            } else {
                None
            }
        })
        .take(3)
        .collect()
}

#[must_use]
pub fn help_groups(context: BindingContext) -> BTreeMap<&'static str, Vec<&'static str>> {
    let mut groups = BTreeMap::new();
    let active_scopes = help_scopes(context);
    let mut seen = BTreeSet::new();
    for binding in bindings()
        .iter()
        .filter(|binding| active_scopes.contains(&binding.scope))
        .filter(|binding| seen.insert((binding.kind, binding.label)))
    {
        let bucket = match binding.kind {
            BindingKind::Standard => "Standard",
            BindingKind::Expert => "Expert aliases",
        };
        groups
            .entry(bucket)
            .or_insert_with(Vec::new)
            .push(binding.label);
    }
    groups
}

#[must_use]
pub fn bindings() -> &'static [Keybinding] {
    KEYBINDINGS.get_or_init(build_bindings)
}

fn resolve_scopes(context: BindingContext) -> Vec<BindingScope> {
    if let Some(transient) = active_transient_scope(context) {
        return vec![BindingScope::Transient(transient), BindingScope::Always];
    }

    vec![
        BindingScope::ScreenRegion(context.active_screen, context.focused_region),
        BindingScope::Region(context.focused_region),
        BindingScope::Screen(context.active_screen),
        BindingScope::Global,
        BindingScope::Always,
    ]
}

fn help_scopes(context: BindingContext) -> Vec<BindingScope> {
    let mut scopes = Vec::new();
    if let Some(transient) = active_transient_scope(context) {
        scopes.push(BindingScope::Transient(transient));
        scopes.push(BindingScope::Always);
        return scopes;
    }
    scopes.extend([
        BindingScope::ScreenRegion(context.active_screen, context.focused_region),
        BindingScope::Region(context.focused_region),
        BindingScope::Screen(context.active_screen),
        BindingScope::Global,
        BindingScope::Always,
    ]);
    scopes
}

const fn active_transient_scope(context: BindingContext) -> Option<TransientLayer> {
    if context.search_open {
        Some(TransientLayer::Search)
    } else if context.help_open {
        Some(TransientLayer::Help)
    } else if context.ai_preflight_open {
        Some(TransientLayer::AiPreflight)
    } else {
        None
    }
}

fn build_bindings() -> Vec<Keybinding> {
    use Action::{
        ActivateFocusedRegion, Back, CloseSearch, ConfirmAiPreflight,
        CycleAiPreflightPrivacyProfile, CyclePatternMetric, DismissAiPreflight, FocusNextRegion,
        FocusPreviousRegion, MoveFocusedRegion, MoveTransientFocus, NextTrendWindow, OpenSearch,
        PreviousTrendWindow, Quit, RefreshRequested, RequestAiComparePreviousSnapshot,
        RequestAiGenerateReport, RequestAiGuidedFollowUp, RequestAiLaunch, RequestAiRerunNextModel,
        RequestAiRerunNextPrivacy, RequestCancelAiRun, RequestJumpToAiEvidence, SearchBackspace,
        SearchNextResult, SearchPreviousResult, ShowScreen, TimelineZoomIn, TimelineZoomOut,
        ToggleHelp, ToggleSessionFilter, ToggleTagFilter, ToggleWorkoutFilter,
    };
    use BindingKind::{Expert, Standard};
    use BindingScope::{Always, Global, Region, ScreenRegion, Transient};
    use ChordKey::{
        BackTab, Backspace, Char, Down, End, Enter, Esc, Home, Left, PageDown, PageUp, Right, Tab,
        Up,
    };
    use FocusRegion::{
        ContextPrimary, ContextSecondary, DashboardActivity, DashboardBreakdown,
        DashboardHeartRate, DashboardHeatmap, DashboardHrv, DashboardReadiness, DashboardRespRate,
        DashboardSleep, DashboardTemp, OpsCoverage, OpsDiagnostics, OpsSummary, OpsWarnings,
        Primary, Secondary, Tertiary, TimelineChart, TimelineControls, TimelineEvents,
        TimelineInspector, TimelineLanes, TopNav, TrendsInspector, TrendsMatrix,
    };
    use TransientLayer::{AiPreflight, Help, Search};

    let mut bindings = vec![
        key(
            Global,
            Standard,
            KeyChord::plain(Tab),
            FocusNextRegion,
            "`Tab` next region",
            true,
        ),
        key(
            Global,
            Standard,
            KeyChord::plain(BackTab),
            FocusPreviousRegion,
            "`Shift+Tab` previous region",
            true,
        ),
        key(
            Global,
            Standard,
            KeyChord::ctrl(Char('f')),
            OpenSearch,
            "`Ctrl+F` find",
            true,
        ),
        key(
            Global,
            Standard,
            KeyChord::plain(Char('?')),
            ToggleHelp,
            "`?` help",
            true,
        ),
        key(
            Always,
            Standard,
            KeyChord::ctrl(Char('c')),
            Quit,
            "`Ctrl+C` quit",
            true,
        ),
        key(
            Global,
            Standard,
            KeyChord::plain(Char('r')),
            RefreshRequested,
            "`r` refresh",
            true,
        ),
        key(
            Global,
            Standard,
            KeyChord::plain(Esc),
            Back,
            "`Esc` back or close",
            true,
        ),
        key(
            Global,
            Standard,
            KeyChord::plain(PageUp),
            MoveFocusedRegion(crate::navigation::NavMove::PageBackward),
            "`PageUp` previous page or day",
            false,
        ),
        key(
            Global,
            Standard,
            KeyChord::plain(PageDown),
            MoveFocusedRegion(crate::navigation::NavMove::PageForward),
            "`PageDown` next page or day",
            false,
        ),
        key(
            Global,
            Expert,
            KeyChord::plain(Char('/')),
            OpenSearch,
            "`/` find",
            false,
        ),
        key(
            Always,
            Expert,
            KeyChord::plain(Char('q')),
            Quit,
            "`q` quit",
            false,
        ),
    ];

    for (index, screen) in Screen::ALL.into_iter().enumerate() {
        let label = match screen {
            Screen::Dashboard => "`1` dashboard",
            Screen::Timeline => "`2` timeline",
            Screen::Trends => "`3` trends",
            Screen::Explain => "`4` explain",
            Screen::Patterns => "`5` patterns",
            Screen::Review => "`6` review",
            Screen::Ai => "`7` AI",
            Screen::Ops => "`8` status",
        };
        bindings.push(key(
            Global,
            Expert,
            KeyChord::plain(Char(SCREEN_DIGITS[index])),
            ShowScreen(screen),
            label,
            false,
        ));
    }

    bindings.extend([
        key(
            Region(TopNav),
            Standard,
            KeyChord::plain(Left),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`Left` previous view",
            true,
        ),
        key(
            Region(TopNav),
            Standard,
            KeyChord::plain(Right),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`Right` next view",
            true,
        ),
        key(
            Region(TopNav),
            Standard,
            KeyChord::plain(Home),
            MoveFocusedRegion(crate::navigation::NavMove::First),
            "`Home` first view",
            false,
        ),
        key(
            Region(TopNav),
            Standard,
            KeyChord::plain(End),
            MoveFocusedRegion(crate::navigation::NavMove::Last),
            "`End` last view",
            false,
        ),
        key(
            Region(TopNav),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` open view",
            true,
        ),
        key(
            Region(TopNav),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` open view",
            false,
        ),
        key(
            Region(TopNav),
            Expert,
            KeyChord::plain(Char('h')),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`h` previous view",
            false,
        ),
        key(
            Region(TopNav),
            Expert,
            KeyChord::plain(Char('l')),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`l` next view",
            false,
        ),
    ]);

    bindings.extend(list_region_bindings(Screen::Timeline, TimelineChart));
    bindings.extend(list_region_bindings(Screen::Timeline, TimelineEvents));
    bindings.extend(list_region_bindings(Screen::Review, Primary));
    bindings.extend(list_region_bindings(Screen::Ai, Primary));
    bindings.extend(list_region_bindings(Screen::Ai, Secondary));
    bindings.extend(list_region_bindings(Screen::Ai, Tertiary));
    bindings.extend(list_region_bindings(Screen::Dashboard, DashboardBreakdown));
    bindings.extend(dashboard_heatmap_list_bindings());
    bindings.extend(list_region_bindings(Screen::Trends, TrendsMatrix));

    bindings.extend(horizontal_region_bindings(Screen::Explain, ContextPrimary));
    bindings.extend(horizontal_region_bindings(Screen::Patterns, ContextPrimary));
    bindings.extend(horizontal_region_bindings(
        Screen::Patterns,
        ContextSecondary,
    ));
    bindings.extend(horizontal_region_bindings(Screen::Review, ContextPrimary));
    bindings.extend(horizontal_region_bindings(Screen::Review, ContextSecondary));
    bindings.extend(horizontal_region_bindings(Screen::Ai, ContextPrimary));

    bindings.extend(horizontal_region_bindings(
        Screen::Timeline,
        TimelineControls,
    ));
    bindings.extend(lateral_region_bindings(Screen::Timeline, TimelineChart));
    bindings.extend(horizontal_region_bindings(Screen::Timeline, TimelineLanes));
    bindings.extend(lateral_region_bindings(
        Screen::Dashboard,
        DashboardBreakdown,
    ));
    bindings.extend(dashboard_heatmap_week_bindings());

    bindings.extend([
        key(
            ScreenRegion(Screen::Ai, Primary),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` launch selected action",
            true,
        ),
        key(
            ScreenRegion(Screen::Ai, Primary),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` launch selected action",
            false,
        ),
        key(
            ScreenRegion(Screen::Review, Primary),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` inspect selected card",
            true,
        ),
        key(
            ScreenRegion(Screen::Review, Primary),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` inspect selected card",
            false,
        ),
        key(
            ScreenRegion(Screen::Timeline, TimelineLanes),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` toggle selected overlay",
            true,
        ),
        key(
            ScreenRegion(Screen::Timeline, TimelineLanes),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` toggle selected overlay",
            false,
        ),
        key(
            ScreenRegion(Screen::Explain, ContextPrimary),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` toggle selected overlay",
            true,
        ),
        key(
            ScreenRegion(Screen::Explain, ContextPrimary),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` toggle selected overlay",
            false,
        ),
        key(
            ScreenRegion(Screen::Patterns, ContextSecondary),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` toggle selected overlay",
            true,
        ),
        key(
            ScreenRegion(Screen::Patterns, ContextSecondary),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` toggle selected overlay",
            false,
        ),
        key(
            ScreenRegion(Screen::Timeline, TimelineEvents),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` inspect selected event",
            true,
        ),
        key(
            ScreenRegion(Screen::Timeline, TimelineEvents),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` inspect selected event",
            false,
        ),
        key(
            ScreenRegion(Screen::Timeline, TimelineInspector),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand selected detail",
            true,
        ),
        key(
            ScreenRegion(Screen::Timeline, TimelineInspector),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` expand selected detail",
            false,
        ),
        key(
            ScreenRegion(Screen::Ai, Secondary),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` inspect selected artifact",
            true,
        ),
        key(
            ScreenRegion(Screen::Ai, Secondary),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` inspect selected artifact",
            false,
        ),
        key(
            ScreenRegion(Screen::Ai, Tertiary),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` activate selected action",
            true,
        ),
        key(
            ScreenRegion(Screen::Ai, Tertiary),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` activate selected action",
            false,
        ),
        key(
            ScreenRegion(Screen::Review, Secondary),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` return to cards",
            false,
        ),
        key(
            ScreenRegion(Screen::Review, Secondary),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` return to cards",
            false,
        ),
        key(
            ScreenRegion(Screen::Timeline, TimelineChart),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand chart",
            true,
        ),
        key(
            ScreenRegion(Screen::Timeline, TimelineChart),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` expand chart",
            false,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardReadiness),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` explain readiness",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardSleep),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` open sleep trends",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardActivity),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` open activity timeline",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardHeartRate),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` open heart-rate trends",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardTemp),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` open temperature trends",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardHrv),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand HRV panel",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardRespRate),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand respiratory panel",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardBreakdown),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand breakdown",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, DashboardHeatmap),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` open selected day",
            true,
        ),
        key(
            ScreenRegion(Screen::Trends, TrendsMatrix),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand trend matrix",
            true,
        ),
        key(
            ScreenRegion(Screen::Trends, TrendsInspector),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand trend detail",
            true,
        ),
        key(
            ScreenRegion(Screen::Ops, OpsSummary),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand summary",
            true,
        ),
        key(
            ScreenRegion(Screen::Ops, OpsCoverage),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand coverage",
            true,
        ),
        key(
            ScreenRegion(Screen::Ops, OpsDiagnostics),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand diagnostics",
            true,
        ),
        key(
            ScreenRegion(Screen::Ops, OpsWarnings),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` expand warnings",
            true,
        ),
    ]);

    bindings.extend([
        key(
            ScreenRegion(Screen::Trends, TrendsMatrix),
            Standard,
            KeyChord::plain(Left),
            PreviousTrendWindow,
            "`Left` previous sort",
            true,
        ),
        key(
            ScreenRegion(Screen::Trends, TrendsMatrix),
            Standard,
            KeyChord::plain(Right),
            NextTrendWindow,
            "`Right` next sort",
            true,
        ),
        key(
            ScreenRegion(Screen::Trends, TrendsMatrix),
            Expert,
            KeyChord::plain(Char('h')),
            PreviousTrendWindow,
            "`h` previous sort",
            false,
        ),
        key(
            ScreenRegion(Screen::Trends, TrendsMatrix),
            Expert,
            KeyChord::plain(Char('l')),
            NextTrendWindow,
            "`l` next sort",
            false,
        ),
    ]);

    bindings.extend([
        key(
            BindingScope::Screen(crate::app::Screen::Timeline),
            Expert,
            KeyChord::plain(Char('-')),
            TimelineZoomOut,
            "`-` zoom out timeline",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Timeline),
            Expert,
            KeyChord::plain(Char('=')),
            TimelineZoomIn,
            "`=` zoom in timeline",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Timeline),
            Expert,
            KeyChord::plain(Char('w')),
            ToggleWorkoutFilter,
            "`w` toggle workout filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Timeline),
            Expert,
            KeyChord::plain(Char('t')),
            ToggleTagFilter,
            "`t` toggle tag filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Timeline),
            Expert,
            KeyChord::plain(Char('s')),
            ToggleSessionFilter,
            "`s` toggle session filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Explain),
            Expert,
            KeyChord::plain(Char('w')),
            ToggleWorkoutFilter,
            "`w` toggle workout filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Explain),
            Expert,
            KeyChord::plain(Char('t')),
            ToggleTagFilter,
            "`t` toggle tag filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Explain),
            Expert,
            KeyChord::plain(Char('s')),
            ToggleSessionFilter,
            "`s` toggle session filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Patterns),
            Expert,
            KeyChord::plain(Char('w')),
            ToggleWorkoutFilter,
            "`w` toggle workout filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Patterns),
            Expert,
            KeyChord::plain(Char('t')),
            ToggleTagFilter,
            "`t` toggle tag filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Patterns),
            Expert,
            KeyChord::plain(Char('s')),
            ToggleSessionFilter,
            "`s` toggle session filter",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Patterns),
            Expert,
            KeyChord::plain(Char('m')),
            CyclePatternMetric,
            "`m` cycle pattern metric",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Dashboard),
            Expert,
            KeyChord::plain(Char('a')),
            RequestAiLaunch(crate::app::AiLaunchIntent::ReviewSelectedDay),
            "`a` review selected day",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Explain),
            Expert,
            KeyChord::plain(Char('a')),
            RequestAiLaunch(crate::app::AiLaunchIntent::ReviewSelectedDay),
            "`a` review selected day",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Review),
            Expert,
            KeyChord::plain(Char('a')),
            RequestAiLaunch(crate::app::AiLaunchIntent::ReviewSelectedDay),
            "`a` review selected day",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('a')),
            RequestAiLaunch(crate::app::AiLaunchIntent::ReviewSelectedDay),
            "`a` review selected day",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Dashboard),
            Expert,
            KeyChord::plain(Char('c')),
            RequestAiLaunch(crate::app::AiLaunchIntent::CompareSelectedWeek),
            "`c` compare selected week",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Patterns),
            Expert,
            KeyChord::plain(Char('c')),
            RequestAiLaunch(crate::app::AiLaunchIntent::CompareSelectedWeek),
            "`c` compare selected week",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Review),
            Expert,
            KeyChord::plain(Char('c')),
            RequestAiLaunch(crate::app::AiLaunchIntent::CompareSelectedWeek),
            "`c` compare selected week",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('c')),
            RequestAiLaunch(crate::app::AiLaunchIntent::CompareSelectedWeek),
            "`c` compare selected week",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('x')),
            RequestCancelAiRun,
            "`x` cancel selected run",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('e')),
            RequestAiGuidedFollowUp(crate::ai::GuidedFollowUpKind::ExpandEvidence),
            "`e` expand evidence",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('y')),
            RequestAiGuidedFollowUp(crate::ai::GuidedFollowUpKind::ShowCounterevidence),
            "`y` show counterevidence",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('i')),
            RequestAiGuidedFollowUp(crate::ai::GuidedFollowUpKind::ExplainRanking),
            "`i` explain ranking",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('d')),
            RequestAiGuidedFollowUp(crate::ai::GuidedFollowUpKind::SuggestLocalDrilldown),
            "`d` suggest drilldown",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('g')),
            RequestAiGenerateReport,
            "`g` generate report",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('u')),
            RequestAiRerunNextPrivacy,
            "`u` rerun with next privacy profile",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('m')),
            RequestAiRerunNextModel,
            "`m` rerun with next model",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('b')),
            RequestAiComparePreviousSnapshot,
            "`b` compare previous snapshot",
            false,
        ),
        key(
            BindingScope::Screen(crate::app::Screen::Ai),
            Expert,
            KeyChord::plain(Char('o')),
            RequestJumpToAiEvidence,
            "`o` open saved evidence",
            false,
        ),
    ]);

    bindings.extend([
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Tab),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Tab` next search focus",
            true,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(BackTab),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Shift+Tab` previous search focus",
            true,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Left),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Left` previous search focus",
            false,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Right),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Right` next search focus",
            false,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Home),
            MoveTransientFocus(crate::navigation::NavMove::First),
            "`Home` first search focus",
            false,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(End),
            MoveTransientFocus(crate::navigation::NavMove::Last),
            "`End` last search focus",
            false,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Up),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Up` previous search focus",
            false,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Down),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Down` next search focus",
            false,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Enter),
            SearchNextResult,
            "`Enter` next search result",
            true,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::shift(Enter),
            SearchPreviousResult,
            "`Shift+Enter` previous search result",
            true,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Esc),
            CloseSearch,
            "`Esc` close search",
            true,
        ),
        key(
            Transient(Search),
            Standard,
            KeyChord::plain(Backspace),
            SearchBackspace,
            "`Backspace` delete",
            false,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(Tab),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Tab` next help focus",
            true,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(BackTab),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Shift+Tab` previous help focus",
            true,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(Left),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Left` previous help focus",
            false,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(Right),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Right` next help focus",
            false,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(Home),
            MoveTransientFocus(crate::navigation::NavMove::First),
            "`Home` first help focus",
            false,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(End),
            MoveTransientFocus(crate::navigation::NavMove::Last),
            "`End` last help focus",
            false,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(Up),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Up` previous help focus",
            false,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(Down),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Down` next help focus",
            false,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(Esc),
            ToggleHelp,
            "`Esc` close help",
            true,
        ),
        key(
            Transient(Help),
            Standard,
            KeyChord::plain(Char('?')),
            ToggleHelp,
            "`?` close help",
            false,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Tab),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Tab` next control",
            true,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(BackTab),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Shift+Tab` previous control",
            true,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Left),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Left` previous control",
            false,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Right),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Right` next control",
            false,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Home),
            MoveTransientFocus(crate::navigation::NavMove::First),
            "`Home` first control",
            false,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(End),
            MoveTransientFocus(crate::navigation::NavMove::Last),
            "`End` last control",
            false,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Up),
            MoveTransientFocus(crate::navigation::NavMove::Previous),
            "`Up` previous control",
            false,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Down),
            MoveTransientFocus(crate::navigation::NavMove::Next),
            "`Down` next control",
            false,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Enter),
            ActivateFocusedRegion,
            "`Enter` activate",
            true,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Char(' ')),
            ActivateFocusedRegion,
            "`Space` activate",
            false,
        ),
        key(
            Transient(AiPreflight),
            Standard,
            KeyChord::plain(Esc),
            DismissAiPreflight,
            "`Esc` cancel",
            true,
        ),
        key(
            Transient(AiPreflight),
            Expert,
            KeyChord::plain(Char('p')),
            CycleAiPreflightPrivacyProfile,
            "`p` rotate privacy",
            false,
        ),
        key(
            Transient(AiPreflight),
            Expert,
            KeyChord::plain(Char('n')),
            DismissAiPreflight,
            "`n` cancel",
            false,
        ),
        key(
            Transient(AiPreflight),
            Expert,
            KeyChord::plain(Char('c')),
            ConfirmAiPreflight,
            "`c` confirm",
            false,
        ),
    ]);

    bindings
}

fn horizontal_region_bindings(screen: Screen, region: FocusRegion) -> Vec<Keybinding> {
    use Action::MoveFocusedRegion;
    use BindingKind::{Expert, Standard};
    use BindingScope::ScreenRegion;
    use ChordKey::{Char, End, Home, Left, Right};

    vec![
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(Left),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`Left` previous option",
            true,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(Right),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`Right` next option",
            true,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(Home),
            MoveFocusedRegion(crate::navigation::NavMove::First),
            "`Home` first option",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(End),
            MoveFocusedRegion(crate::navigation::NavMove::Last),
            "`End` last option",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Expert,
            KeyChord::plain(Char('h')),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`h` previous option",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Expert,
            KeyChord::plain(Char('l')),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`l` next option",
            false,
        ),
    ]
}

fn dashboard_heatmap_week_bindings() -> Vec<Keybinding> {
    use Action::MoveFocusedRegion;
    use BindingKind::{Expert, Standard};
    use BindingScope::ScreenRegion;
    use ChordKey::{Char, Left, Right};

    vec![
        key(
            ScreenRegion(Screen::Dashboard, FocusRegion::DashboardHeatmap),
            Standard,
            KeyChord::plain(Left),
            MoveFocusedRegion(crate::navigation::NavMove::PageBackward),
            "`Left` older week",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, FocusRegion::DashboardHeatmap),
            Standard,
            KeyChord::plain(Right),
            MoveFocusedRegion(crate::navigation::NavMove::PageForward),
            "`Right` newer week",
            true,
        ),
        key(
            ScreenRegion(Screen::Dashboard, FocusRegion::DashboardHeatmap),
            Expert,
            KeyChord::plain(Char('h')),
            MoveFocusedRegion(crate::navigation::NavMove::PageBackward),
            "`h` older week",
            false,
        ),
        key(
            ScreenRegion(Screen::Dashboard, FocusRegion::DashboardHeatmap),
            Expert,
            KeyChord::plain(Char('l')),
            MoveFocusedRegion(crate::navigation::NavMove::PageForward),
            "`l` newer week",
            false,
        ),
    ]
}

fn dashboard_heatmap_list_bindings() -> Vec<Keybinding> {
    use Action::MoveFocusedRegion;
    use BindingKind::{Expert, Standard};
    use BindingScope::ScreenRegion;
    use ChordKey::{Char, Down, End, Home, PageDown, PageUp, Up};

    let scope = ScreenRegion(Screen::Dashboard, FocusRegion::DashboardHeatmap);
    vec![
        key(
            scope,
            Standard,
            KeyChord::plain(Up),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`Up` previous day",
            true,
        ),
        key(
            scope,
            Standard,
            KeyChord::plain(Down),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`Down` next day",
            true,
        ),
        key(
            scope,
            Standard,
            KeyChord::plain(Home),
            MoveFocusedRegion(crate::navigation::NavMove::First),
            "`Home` earliest day",
            false,
        ),
        key(
            scope,
            Standard,
            KeyChord::plain(End),
            MoveFocusedRegion(crate::navigation::NavMove::Last),
            "`End` latest day",
            false,
        ),
        key(
            scope,
            Standard,
            KeyChord::plain(PageUp),
            MoveFocusedRegion(crate::navigation::NavMove::PageBackward),
            "`PageUp` older week",
            false,
        ),
        key(
            scope,
            Standard,
            KeyChord::plain(PageDown),
            MoveFocusedRegion(crate::navigation::NavMove::PageForward),
            "`PageDown` newer week",
            false,
        ),
        key(
            scope,
            Expert,
            KeyChord::plain(Char('k')),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`k` previous day",
            false,
        ),
        key(
            scope,
            Expert,
            KeyChord::plain(Char('j')),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`j` next day",
            false,
        ),
        key(
            scope,
            Expert,
            KeyChord::plain(Char('g')),
            MoveFocusedRegion(crate::navigation::NavMove::First),
            "`g` earliest day",
            false,
        ),
        key(
            scope,
            Expert,
            KeyChord::shift(Char('g')),
            MoveFocusedRegion(crate::navigation::NavMove::Last),
            "`G` latest day",
            false,
        ),
    ]
}

fn lateral_region_bindings(screen: Screen, region: FocusRegion) -> Vec<Keybinding> {
    use Action::MoveFocusedRegion;
    use BindingKind::{Expert, Standard};
    use BindingScope::ScreenRegion;
    use ChordKey::{Char, Left, Right};

    vec![
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(Left),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`Left` previous option",
            true,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(Right),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`Right` next option",
            true,
        ),
        key(
            ScreenRegion(screen, region),
            Expert,
            KeyChord::plain(Char('h')),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`h` previous option",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Expert,
            KeyChord::plain(Char('l')),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`l` next option",
            false,
        ),
    ]
}

fn list_region_bindings(screen: Screen, region: FocusRegion) -> Vec<Keybinding> {
    use Action::MoveFocusedRegion;
    use BindingKind::{Expert, Standard};
    use BindingScope::ScreenRegion;
    use ChordKey::{Char, Down, End, Home, PageDown, PageUp, Up};

    let mut bindings = vec![
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(Up),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`Up` previous item",
            true,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(Down),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`Down` next item",
            true,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(Home),
            MoveFocusedRegion(crate::navigation::NavMove::First),
            "`Home` first item",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(End),
            MoveFocusedRegion(crate::navigation::NavMove::Last),
            "`End` last item",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(PageUp),
            MoveFocusedRegion(crate::navigation::NavMove::PageBackward),
            "`PageUp` jump back",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Standard,
            KeyChord::plain(PageDown),
            MoveFocusedRegion(crate::navigation::NavMove::PageForward),
            "`PageDown` jump forward",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Expert,
            KeyChord::plain(Char('k')),
            MoveFocusedRegion(crate::navigation::NavMove::Previous),
            "`k` previous item",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Expert,
            KeyChord::plain(Char('j')),
            MoveFocusedRegion(crate::navigation::NavMove::Next),
            "`j` next item",
            false,
        ),
        key(
            ScreenRegion(screen, region),
            Expert,
            KeyChord::shift(Char('g')),
            MoveFocusedRegion(crate::navigation::NavMove::Last),
            "`G` last item",
            false,
        ),
    ];
    if screen != Screen::Ai {
        bindings.push(key(
            ScreenRegion(screen, region),
            Expert,
            KeyChord::plain(Char('g')),
            MoveFocusedRegion(crate::navigation::NavMove::First),
            "`g` first item",
            false,
        ));
    }
    bindings
}

const fn key(
    scope: BindingScope,
    kind: BindingKind,
    chord: KeyChord,
    action: Action,
    label: &'static str,
    show_in_footer: bool,
) -> Keybinding {
    Keybinding {
        scope,
        kind,
        chord: chord.normalized(),
        action,
        label,
        show_in_footer,
    }
}

impl KeyChord {
    #[must_use]
    const fn normalized(self) -> Self {
        match self.code {
            ChordKey::Char(mut character) => {
                let mut modifiers = self.modifiers;
                if modifiers.control {
                    if character.is_ascii_uppercase() {
                        character = character.to_ascii_lowercase();
                    }
                    modifiers.shift = false;
                } else if modifiers.shift {
                    if character.is_ascii_lowercase() {
                        character = character.to_ascii_uppercase();
                    }
                    modifiers.shift = false;
                }

                Self {
                    code: ChordKey::Char(character),
                    modifiers,
                }
            }
            ChordKey::BackTab => Self {
                code: ChordKey::BackTab,
                modifiers: ChordModifiers {
                    control: self.modifiers.control,
                    shift: false,
                    alt: self.modifiers.alt,
                },
            },
            _ => self,
        }
    }

    #[must_use]
    pub const fn plain(code: ChordKey) -> Self {
        Self {
            code,
            modifiers: ChordModifiers {
                control: false,
                shift: false,
                alt: false,
            },
        }
        .normalized()
    }

    #[must_use]
    pub const fn ctrl(code: ChordKey) -> Self {
        Self {
            code,
            modifiers: ChordModifiers {
                control: true,
                shift: false,
                alt: false,
            },
        }
        .normalized()
    }

    #[must_use]
    pub const fn shift(code: ChordKey) -> Self {
        Self {
            code,
            modifiers: ChordModifiers {
                control: false,
                shift: true,
                alt: false,
            },
        }
        .normalized()
    }

    const fn from_key_event(event: KeyEvent) -> Option<Self> {
        let code = match event.code {
            KeyCode::Char(character) => ChordKey::Char(character),
            KeyCode::Tab => ChordKey::Tab,
            KeyCode::BackTab => ChordKey::BackTab,
            KeyCode::Left => ChordKey::Left,
            KeyCode::Right => ChordKey::Right,
            KeyCode::Up => ChordKey::Up,
            KeyCode::Down => ChordKey::Down,
            KeyCode::Enter => ChordKey::Enter,
            KeyCode::Esc => ChordKey::Esc,
            KeyCode::Home => ChordKey::Home,
            KeyCode::End => ChordKey::End,
            KeyCode::PageUp => ChordKey::PageUp,
            KeyCode::PageDown => ChordKey::PageDown,
            KeyCode::Backspace => ChordKey::Backspace,
            _ => return None,
        };
        Some(
            Self {
                code,
                modifiers: ChordModifiers {
                    control: event.modifiers.contains(KeyModifiers::CONTROL),
                    shift: event.modifiers.contains(KeyModifiers::SHIFT),
                    alt: event.modifiers.contains(KeyModifiers::ALT),
                },
            }
            .normalized(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{BindingContext, BindingKind, BindingScope, bindings, footer_hints, help_groups};
    use crate::action::Action;
    use crate::app::Screen;
    use crate::navigation::FocusRegion;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn registry_contains_no_scope_local_collisions() {
        let bindings = bindings();
        for (index, binding) in bindings.iter().enumerate() {
            let duplicate = bindings
                .iter()
                .enumerate()
                .find(|(candidate_index, candidate)| {
                    *candidate_index != index
                        && candidate.scope == binding.scope
                        && candidate.chord == binding.chord
                });
            assert!(
                duplicate.is_none(),
                "found collision for {:?}",
                binding.label
            );
        }
    }

    #[test]
    fn footer_hints_surface_standard_bindings_only() {
        let hints = footer_hints(
            BindingContext {
                active_screen: Screen::Timeline,
                focused_region: FocusRegion::TimelineChart,
                search_open: false,
                help_open: false,
                ai_preflight_open: false,
            },
            true,
        );
        assert!(hints.iter().all(|hint| !hint.contains("`j`")));
        assert!(hints.iter().any(|hint| hint.contains("`Ctrl+F`")));
    }

    #[test]
    fn footer_hints_hide_activation_copy_when_enter_is_not_truthful() {
        let hints = footer_hints(
            BindingContext {
                active_screen: Screen::Ai,
                focused_region: FocusRegion::Tertiary,
                search_open: false,
                help_open: false,
                ai_preflight_open: false,
            },
            false,
        );

        assert!(!hints.iter().any(|hint| hint.contains("Enter")));
    }

    #[test]
    fn help_groups_split_standard_and_expert_entries() {
        let groups = help_groups(BindingContext {
            active_screen: Screen::Ai,
            focused_region: FocusRegion::Secondary,
            search_open: false,
            help_open: false,
            ai_preflight_open: false,
        });
        assert!(groups.contains_key("Standard"));
        assert!(groups.contains_key("Expert aliases"));
    }

    #[test]
    fn global_standard_bindings_cover_region_navigation() {
        let globals = bindings()
            .iter()
            .filter(|binding| binding.scope == BindingScope::Global)
            .filter(|binding| binding.kind == BindingKind::Standard)
            .count();
        assert!(globals >= 5);
    }

    #[test]
    fn transient_scope_takes_precedence_over_region_scope() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Review,
                focused_region: FocusRegion::Primary,
                search_open: true,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, Some(Action::SearchNextResult));
    }

    #[test]
    fn search_modal_traps_tab_navigation_inside_the_overlay() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Review,
                focused_region: FocusRegion::Primary,
                search_open: true,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(
            action,
            Some(Action::MoveTransientFocus(crate::navigation::NavMove::Next))
        );
    }

    #[test]
    fn transients_do_not_fall_through_to_screen_shortcuts() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Ai,
                focused_region: FocusRegion::Secondary,
                search_open: true,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, None);
    }

    #[test]
    fn search_help_groups_follow_the_visible_transient_scope() {
        let groups = help_groups(BindingContext {
            active_screen: Screen::Ai,
            focused_region: FocusRegion::Secondary,
            search_open: true,
            help_open: false,
            ai_preflight_open: false,
        });

        let standard = groups
            .get("Standard")
            .unwrap_or_else(|| panic!("standard help group should exist"));

        assert!(
            standard
                .iter()
                .any(|label| label.contains("next search focus"))
        );
        assert!(standard.iter().any(|label| label.contains("close search")));
        assert!(!standard.iter().any(|label| label.contains("cancel")));
    }

    #[test]
    fn transient_scopes_still_allow_global_quit() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            BindingContext {
                active_screen: Screen::Ai,
                focused_region: FocusRegion::Secondary,
                search_open: true,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn help_modal_traps_tab_navigation_inside_the_overlay() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Explain,
                focused_region: FocusRegion::ContextPrimary,
                search_open: false,
                help_open: true,
                ai_preflight_open: false,
            },
        );

        assert_eq!(
            action,
            Some(Action::MoveTransientFocus(crate::navigation::NavMove::Next))
        );
    }

    #[test]
    fn help_modal_escape_resolves_to_close_help() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Dashboard,
                focused_region: FocusRegion::DashboardReadiness,
                search_open: false,
                help_open: true,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, Some(Action::ToggleHelp));
    }

    #[test]
    fn help_modal_blocks_background_refresh_shortcuts() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Dashboard,
                focused_region: FocusRegion::DashboardReadiness,
                search_open: false,
                help_open: true,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, None);
    }

    #[test]
    fn ai_preflight_uses_transient_focus_actions_for_tabbing() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            BindingContext {
                active_screen: Screen::Ai,
                focused_region: FocusRegion::Tertiary,
                search_open: false,
                help_open: false,
                ai_preflight_open: true,
            },
        );

        assert_eq!(
            action,
            Some(Action::MoveTransientFocus(
                crate::navigation::NavMove::Previous
            ))
        );
    }

    #[test]
    fn visible_transient_overrides_preflight_scope_resolution() {
        let help_action = super::resolve(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Ai,
                focused_region: FocusRegion::Secondary,
                search_open: false,
                help_open: true,
                ai_preflight_open: true,
            },
        );
        assert_eq!(help_action, Some(Action::ToggleHelp));

        let search_action = super::resolve(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Ai,
                focused_region: FocusRegion::Secondary,
                search_open: true,
                help_open: false,
                ai_preflight_open: true,
            },
        );
        assert_eq!(search_action, Some(Action::SearchNextResult));
    }

    #[test]
    fn help_groups_follow_the_visible_transient_scope() {
        let groups = help_groups(BindingContext {
            active_screen: Screen::Ai,
            focused_region: FocusRegion::Secondary,
            search_open: false,
            help_open: true,
            ai_preflight_open: true,
        });

        let standard = groups
            .get("Standard")
            .unwrap_or_else(|| panic!("standard help group should exist"));

        assert!(standard.iter().any(|label| label.contains("close help")));
        assert!(!standard.iter().any(|label| label.contains("cancel")));
        assert!(!standard.iter().any(|label| label.contains("activate")));
    }

    #[test]
    fn ai_screen_report_shortcut_is_not_shadowed_by_list_aliases() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Ai,
                focused_region: FocusRegion::Secondary,
                search_open: false,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, Some(Action::RequestAiGenerateReport));
    }

    #[test]
    fn shifted_character_bindings_match_uppercase_terminal_events() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE),
            BindingContext {
                active_screen: Screen::Review,
                focused_region: FocusRegion::Primary,
                search_open: false,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(
            action,
            Some(Action::MoveFocusedRegion(crate::navigation::NavMove::Last))
        );
    }

    #[test]
    fn shifted_symbol_bindings_match_terminals_that_keep_shift_modifier() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
            BindingContext {
                active_screen: Screen::Dashboard,
                focused_region: FocusRegion::TopNav,
                search_open: false,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, Some(Action::ToggleHelp));
    }

    #[test]
    fn backtab_bindings_match_terminals_that_keep_shift_modifier() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            BindingContext {
                active_screen: Screen::Dashboard,
                focused_region: FocusRegion::TopNav,
                search_open: false,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, Some(Action::FocusPreviousRegion));
    }

    #[test]
    fn alt_modified_keys_do_not_fall_through_to_plain_bindings() {
        let action = super::resolve(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT),
            BindingContext {
                active_screen: Screen::Dashboard,
                focused_region: FocusRegion::TopNav,
                search_open: false,
                help_open: false,
                ai_preflight_open: false,
            },
        );

        assert_eq!(action, None);
    }

    #[test]
    fn dashboard_heatmap_week_shortcuts_page_by_week() {
        let context = BindingContext {
            active_screen: Screen::Dashboard,
            focused_region: FocusRegion::DashboardHeatmap,
            search_open: false,
            help_open: false,
            ai_preflight_open: false,
        };

        for key in [
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        ] {
            assert_eq!(
                super::resolve(key, context),
                Some(Action::MoveFocusedRegion(
                    crate::navigation::NavMove::PageBackward
                ))
            );
        }

        for key in [
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
        ] {
            assert_eq!(
                super::resolve(key, context),
                Some(Action::MoveFocusedRegion(
                    crate::navigation::NavMove::PageForward
                ))
            );
        }
    }
}
