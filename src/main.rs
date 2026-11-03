use gtk4::gdk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::prelude::*;
use std::cell::Cell;
use std::collections::HashMap;
use std::process::Command;
use std::thread;
use std::time::Duration;

mod core;
use crate::core::{
    AppConfig, NetworkManager, QrGenerator, SystemdManager, TailscaleInfo, TailscaleManager,
    ZellijManager, ZellijSessionEntry,
};

struct AppModel {
    config: AppConfig,
    is_running: bool,
    is_creating: bool,
    progress_fraction: f64,
    status_log: Vec<String>,
    tailscale_info: TailscaleInfo,
    current_url: String,
    password_input: String,
    new_session_name: String,
    sessions: Vec<ZellijSessionEntry>,
    observer_tokens: HashMap<String, String>,
    observer_status_text: String,
    sessions_dirty: Cell<bool>,
}

#[derive(Debug)]
enum AppMsg {
    StartCreation,
    ProgressUpdate(f64, String),
    CreationFinished(bool, String),
    StopService,
    ToggleServeToWeb(bool),
    TogglePassword(bool),
    PasswordChanged(String),
    ToggleAutoStart(bool),
    PortChanged(u16),
    CopyUrl,
    OpenBrowser,
    ShowQrModal,
    ConnectTailscale,
    RefreshStatus,
    RequestFreshReset,
    // Sessions management
    SessionsLoaded(Vec<ZellijSessionEntry>),
    GenerateObserverToken(String),
    ObserverTokenGenerated(String, String),
    ShowObserverQr(String),
    DeleteSession(String),
    ResurrectSession(String),
    NewSessionNameChanged(String),
    CreateNewSession,
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Scry"),
            set_default_width: 580,
            set_default_height: 740,

            #[name = "toolbar"]
            adw::ToolbarView {
                #[wrap(Some)]
                #[name = "stack"]
                set_content = &adw::ViewStack {
                    // ==========================================
                    // TAB 1: TERMINAL CONTROL & CONFIGURATION
                    // ==========================================
                    #[name = "page_terminal"]
                    add = &adw::PreferencesPage {
                        // Section 1: Active Terminal Status
                        adw::PreferencesGroup {
                            set_title: "Active Web Terminal",
                            #[watch]
                            set_visible: model.is_running && !model.is_creating,

                            adw::ActionRow {
                                set_title: "Status: Online",
                                #[watch]
                                set_subtitle: &format!("Session: {} | Port: {}", model.config.session_name, model.config.port),

                                add_prefix = &gtk4::Image {
                                    set_icon_name: Some("network-wireless-symbolic"),
                                    add_css_class: "success",
                                    set_valign: gtk4::Align::Center,
                                },

                                add_suffix = &gtk4::Button {
                                    set_label: "Stop Terminal",
                                    add_css_class: "destructive-action",
                                    set_valign: gtk4::Align::Center,
                                    connect_clicked => AppMsg::StopService,
                                }
                            },

                            adw::ActionRow {
                                set_title: "Access Address",
                                #[watch]
                                set_subtitle: &model.current_url,

                                add_suffix = &gtk4::Box {
                                    set_orientation: gtk4::Orientation::Horizontal,
                                    set_spacing: 6,
                                    set_valign: gtk4::Align::Center,

                                    gtk4::Button {
                                        set_icon_name: "edit-copy-symbolic",
                                        set_tooltip_text: Some("Copy URL"),
                                        add_css_class: "flat",
                                        set_valign: gtk4::Align::Center,
                                        connect_clicked => AppMsg::CopyUrl,
                                    },
                                    gtk4::Button {
                                        set_icon_name: "camera-photo-symbolic",
                                        set_tooltip_text: Some("Scan QR with Phone"),
                                        add_css_class: "flat",
                                        set_valign: gtk4::Align::Center,
                                        connect_clicked => AppMsg::ShowQrModal,
                                    },
                                    gtk4::Button {
                                        set_icon_name: "web-browser-symbolic",
                                        set_tooltip_text: Some("Open in Browser"),
                                        add_css_class: "flat",
                                        set_valign: gtk4::Align::Center,
                                        connect_clicked => AppMsg::OpenBrowser,
                                    }
                                }
                            },

                            adw::ActionRow {
                                set_title: "Observer / View-Only Access",
                                #[watch]
                                set_subtitle: &model.observer_status_text,

                                add_suffix = &gtk4::Box {
                                    set_orientation: gtk4::Orientation::Horizontal,
                                    set_spacing: 6,
                                    set_valign: gtk4::Align::Center,

                                    gtk4::Button {
                                        #[watch]
                                        set_visible: model.observer_tokens.contains_key(&model.config.session_name),
                                        set_icon_name: "camera-photo-symbolic",
                                        set_tooltip_text: Some("View Observer QR Code"),
                                        add_css_class: "flat",
                                        set_valign: gtk4::Align::Center,
                                        connect_clicked[sender, sess = model.config.session_name.clone()] => move |_| {
                                            sender.input(AppMsg::ShowObserverQr(sess.clone()));
                                        }
                                    },

                                    gtk4::Button {
                                        #[watch]
                                        set_label: if model.observer_tokens.contains_key(&model.config.session_name) {
                                            "Regenerate"
                                        } else {
                                            "Generate Link"
                                        },
                                        add_css_class: "suggested-action",
                                        set_valign: gtk4::Align::Center,
                                        connect_clicked[sender, sess = model.config.session_name.clone()] => move |_| {
                                            sender.input(AppMsg::GenerateObserverToken(sess.clone()));
                                        }
                                    }
                                }
                            }
                        },

                        // Section 2: Creation in Progress
                        adw::PreferencesGroup {
                            set_title: "Setting Up Terminal...",
                            #[watch]
                            set_visible: model.is_creating,

                            adw::ActionRow {
                                set_title: "Configuring Services",
                                #[watch]
                                set_subtitle: if model.progress_fraction >= 1.0 {
                                    "Done!"
                                } else {
                                    "Preparing background services..."
                                },
                                add_prefix = &gtk4::Spinner {
                                    #[watch]
                                    set_spinning: model.is_creating,
                                    set_valign: gtk4::Align::Center,
                                }
                            },

                            adw::PreferencesRow {
                                set_activatable: false,
                                #[wrap(Some)]
                                set_child = &gtk4::Box {
                                    set_orientation: gtk4::Orientation::Vertical,
                                    set_spacing: 10,
                                    set_margin_top: 12,
                                    set_margin_bottom: 12,
                                    set_margin_start: 16,
                                    set_margin_end: 16,

                                    gtk4::ProgressBar {
                                        set_hexpand: true,
                                        #[watch]
                                        set_fraction: model.progress_fraction,
                                        set_show_text: false,
                                    },

                                    gtk4::ScrolledWindow {
                                        set_min_content_height: 130,
                                        set_max_content_height: 180,
                                        set_hscrollbar_policy: gtk4::PolicyType::Never,

                                        gtk4::Label {
                                            set_margin_all: 10,
                                            set_wrap: true,
                                            set_xalign: 0.0,
                                            set_yalign: 0.0,
                                            add_css_class: "monospace",
                                            add_css_class: "caption",
                                            #[watch]
                                            set_label: &model.status_log.join("\n"),
                                        }
                                    }
                                }
                            }
                        },

                        // Section 3: Network Exposure (Offline)
                        adw::PreferencesGroup {
                            set_title: "Network Exposure",
                            #[watch]
                            set_visible: !model.is_running && !model.is_creating,

                            #[name(serve_to_web_row)]
                            adw::SwitchRow {
                                set_title: "Serve to Web",
                                set_subtitle: "Access from anywhere via secure Tailscale tunnel. Off = Local LAN only.",
                                #[track = "model.config.serve_to_web != serve_to_web_row.is_active()"]
                                set_active: model.config.serve_to_web,
                                connect_active_notify[sender] => move |row| {
                                    sender.input(AppMsg::ToggleServeToWeb(row.is_active()));
                                }
                            },

                            adw::ActionRow {
                                set_title: "Tailscale Login Required",
                                set_subtitle: "Log in to Tailscale to enable public internet routing",
                                #[watch]
                                set_visible: model.config.serve_to_web && !model.tailscale_info.is_logged_in,

                                add_suffix = &gtk4::Button {
                                    set_label: "Connect Tailscale",
                                    add_css_class: "suggested-action",
                                    set_valign: gtk4::Align::Center,
                                    connect_clicked => AppMsg::ConnectTailscale,
                                }
                            }
                        },

                        // Section 4: Security (Offline)
                        adw::PreferencesGroup {
                            set_title: "Security and Authentication",
                            #[watch]
                            set_visible: !model.is_running && !model.is_creating,

                            #[name(password_protection_row)]
                            adw::SwitchRow {
                                set_title: "Password Protection",
                                #[watch]
                                set_subtitle: if model.config.serve_to_web {
                                    "Required when serving to the web"
                                } else {
                                    "Require a password to connect from any browser"
                                },
                                #[watch]
                                set_sensitive: !model.config.serve_to_web,
                                #[track = "(model.config.require_password || model.config.serve_to_web) != password_protection_row.is_active()"]
                                set_active: model.config.require_password || model.config.serve_to_web,
                                connect_active_notify[sender] => move |row| {
                                    sender.input(AppMsg::TogglePassword(row.is_active()));
                                }
                            },

                            #[name(password_entry)]
                            adw::PasswordEntryRow {
                                set_title: "Terminal Password",
                                #[watch]
                                set_visible: model.config.require_password || model.config.serve_to_web,
                                #[track = "model.password_input != password_entry.text().as_str()"]
                                set_text: &model.password_input,
                                connect_changed[sender] => move |entry| {
                                    sender.input(AppMsg::PasswordChanged(entry.text().to_string()));
                                }
                            }
                        },

                        // Section 5: Persistence & Advanced (Offline)
                        adw::PreferencesGroup {
                            set_title: "System Persistence",
                            #[watch]
                            set_visible: !model.is_running && !model.is_creating,

                            #[name(boot_row)]
                            adw::SwitchRow {
                                set_title: "Start on System Boot",
                                set_subtitle: "Automatically keep the terminal running after reboot or lid close",
                                #[track = "model.config.auto_start_on_boot != boot_row.is_active()"]
                                set_active: model.config.auto_start_on_boot,
                                connect_active_notify[sender] => move |row| {
                                    sender.input(AppMsg::ToggleAutoStart(row.is_active()));
                                }
                            },

                            adw::ExpanderRow {
                                set_title: "Advanced Configuration",
                                set_subtitle: "Session name, web port selector, and network identifiers",

                                add_row = &adw::ActionRow {
                                    set_title: "Session Name",
                                    #[watch]
                                    set_subtitle: &model.config.session_name,
                                },

                                add_row = &adw::ActionRow {
                                    set_title: "Web Port",
                                    set_subtitle: "Port for the terminal web server (default: 8082)",
                                    #[name(port_spin)]
                                    add_suffix = &gtk4::SpinButton {
                                        set_range: (1024.0, 65535.0),
                                        set_increments: (1.0, 100.0),
                                        set_digits: 0,
                                        set_valign: gtk4::Align::Center,
                                        set_width_chars: 6,
                                        #[track = "model.config.port as f64 != port_spin.value()"]
                                        set_value: model.config.port as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::PortChanged(spin.value() as u16));
                                        }
                                    }
                                },

                                add_row = &adw::ActionRow {
                                    set_title: "Tailscale Node",
                                    #[watch]
                                    set_subtitle: model.tailscale_info.fqdn.as_deref().unwrap_or("Not connected"),
                                }
                            }
                        },

                        // Section 6: Create Button (Offline)
                        adw::PreferencesGroup {
                            #[watch]
                            set_visible: !model.is_running && !model.is_creating,

                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Vertical,
                                set_halign: gtk4::Align::Center,
                                set_margin_top: 8,
                                set_margin_bottom: 16,

                                gtk4::Button {
                                    set_label: "Create Web Terminal",
                                    add_css_class: "suggested-action",
                                    add_css_class: "pill",
                                    set_width_request: 260,
                                    set_height_request: 48,
                                    connect_clicked => AppMsg::StartCreation,
                                }
                            }
                        }
                    },

                    // ==========================================
                    // TAB 2: SESSIONS MANAGER
                    // ==========================================
                    #[name = "page_sessions"]
                    add = &adw::PreferencesPage {
                        adw::PreferencesGroup {
                            set_title: "Terminal Sessions",
                            set_description: Some("Browse active and exited Zellij sessions, share observer links, or delete sessions."),

                            #[wrap(Some)]
                            #[name = "sessions_listbox"]
                            set_header_suffix = &gtk4::Box {
                                set_orientation: gtk4::Orientation::Horizontal,
                            },

                            #[name = "sessions_box"]
                            gtk4::ListBox {
                                add_css_class: "boxed-list",
                            }
                        },

                        adw::PreferencesGroup {
                            set_title: "Create Additional Session",
                            set_description: Some("Start a new named persistent Zellij terminal session in the background."),

                            #[name(new_session_entry)]
                            adw::EntryRow {
                                set_title: "New Session Name",
                                #[track = "model.new_session_name != new_session_entry.text().as_str()"]
                                set_text: &model.new_session_name,
                                connect_changed[sender] => move |entry| {
                                    sender.input(AppMsg::NewSessionNameChanged(entry.text().to_string()));
                                },

                                add_suffix = &gtk4::Button {
                                    set_label: "Create Session",
                                    add_css_class: "suggested-action",
                                    set_valign: gtk4::Align::Center,
                                    connect_clicked => AppMsg::CreateNewSession,
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);

        let config = AppConfig::load();
        let is_running = SystemdManager::is_active();
        let tailscale_info = TailscaleManager::get_info();
        let sessions = ZellijManager::list_all_sessions();

        let current_url = if config.serve_to_web && tailscale_info.fqdn.is_some() {
            NetworkManager::get_web_url(tailscale_info.fqdn.as_ref().unwrap())
        } else {
            NetworkManager::get_lan_url(config.port)
        };

        let pw = config.require_password || config.serve_to_web;
        let password_input = if pw {
            "ByAnyMeans".to_string()
        } else {
            String::new()
        };

        let model = AppModel {
            config,
            is_running,
            is_creating: false,
            progress_fraction: 0.0,
            status_log: Vec::new(),
            tailscale_info,
            current_url,
            password_input,
            new_session_name: "scry-work".to_string(),
            sessions,
            observer_tokens: HashMap::new(),
            observer_status_text: "Generate a read-only passcode to let someone watch your terminal".to_string(),
            sessions_dirty: Cell::new(true),
        };

        let widgets = view_output!();

        // Build header bar with view switcher
        let header = adw::HeaderBar::new();

        let reset_btn = gtk4::Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text("Reset to Fresh State (Test from Scratch)")
            .css_classes(["flat"])
            .valign(gtk4::Align::Center)
            .build();
        {
            let s = sender.clone();
            reset_btn.connect_clicked(move |_| s.input(AppMsg::RequestFreshReset));
        }
        header.pack_start(&reset_btn);

        let refresh_btn = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Refresh Status")
            .css_classes(["flat"])
            .valign(gtk4::Align::Center)
            .build();
        {
            let s = sender.clone();
            refresh_btn.connect_clicked(move |_| s.input(AppMsg::RefreshStatus));
        }
        header.pack_end(&refresh_btn);

        let switcher = adw::ViewSwitcher::new();
        switcher.set_stack(Some(&widgets.stack));
        switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
        header.set_title_widget(Some(&switcher));

        widgets.toolbar.add_top_bar(&header);

        let p1 = widgets.stack.page(&widgets.page_terminal);
        p1.set_name(Some("terminal"));
        p1.set_title(Some("Terminal"));
        p1.set_icon_name(Some("utilities-terminal-symbolic"));

        let p2 = widgets.stack.page(&widgets.page_sessions);
        p2.set_name(Some("sessions"));
        p2.set_title(Some("Sessions"));
        p2.set_icon_name(Some("view-list-symbolic"));

        ComponentParts { model, widgets }
    }

    fn post_view() {
        if !model.sessions_dirty.get() {
            return;
        }
        model.sessions_dirty.set(false);

        // Rebuild sessions list box dynamically
        while let Some(child) = widgets.sessions_box.first_child() {
            widgets.sessions_box.remove(&child);
        }

        if model.sessions.is_empty() {
            let empty_row = adw::ActionRow::builder()
                .title("No Sessions Found")
                .subtitle("Create your first web terminal in the Terminal tab")
                .build();
            widgets.sessions_box.append(&empty_row);
        } else {
            for session in &model.sessions {
                let expander = adw::ExpanderRow::builder()
                    .title(&session.name)
                    .subtitle(if session.is_exited {
                        "Exited (Attach to resurrect)"
                    } else if session.name == model.config.session_name && model.is_running {
                        "Active Web Terminal (Port 8082)"
                    } else {
                        "Running in Background"
                    })
                    .build();

                let icon_name = if session.is_exited {
                    "system-shutdown-symbolic"
                } else {
                    "utilities-terminal-symbolic"
                };
                let icon = gtk4::Image::builder()
                    .icon_name(icon_name)
                    .valign(gtk4::Align::Center)
                    .build();
                if !session.is_exited {
                    icon.add_css_class("success");
                }
                expander.add_prefix(&icon);

                // Observer / View-Only Access sub-row
                let obs_row = adw::ActionRow::builder()
                    .title("Observer / View-Only Access")
                    .build();

                if let Some(token) = model.observer_tokens.get(&session.name) {
                    obs_row.set_subtitle(&format!("Code: {} (Enter at login for view-only)", token));

                    let btn_box = gtk4::Box::builder()
                        .orientation(gtk4::Orientation::Horizontal)
                        .spacing(6)
                        .valign(gtk4::Align::Center)
                        .build();

                    let copy_btn = gtk4::Button::builder()
                        .icon_name("edit-copy-symbolic")
                        .tooltip_text("Copy Code")
                        .css_classes(["flat"])
                        .valign(gtk4::Align::Center)
                        .build();
                    {
                        let tok = token.clone();
                        copy_btn.connect_clicked(move |_| {
                            if let Some(display) = gdk::Display::default() {
                                display.clipboard().set_text(&tok);
                            }
                        });
                    }

                    let qr_btn = gtk4::Button::builder()
                        .icon_name("camera-photo-symbolic")
                        .tooltip_text("View Observer QR Code")
                        .css_classes(["flat"])
                        .valign(gtk4::Align::Center)
                        .build();
                    {
                        let s = sender.clone();
                        let sess = session.name.clone();
                        qr_btn.connect_clicked(move |_| {
                            s.input(AppMsg::ShowObserverQr(sess.clone()));
                        });
                    }

                    let regen_btn = gtk4::Button::builder()
                        .icon_name("view-refresh-symbolic")
                        .tooltip_text("Regenerate Code")
                        .css_classes(["flat"])
                        .valign(gtk4::Align::Center)
                        .build();
                    {
                        let s = sender.clone();
                        let sess = session.name.clone();
                        regen_btn.connect_clicked(move |_| {
                            s.input(AppMsg::GenerateObserverToken(sess.clone()));
                        });
                    }

                    btn_box.append(&copy_btn);
                    btn_box.append(&qr_btn);
                    btn_box.append(&regen_btn);
                    obs_row.add_suffix(&btn_box);
                } else {
                    obs_row.set_subtitle("Allow others to watch terminal output in real time");
                    let gen_btn = gtk4::Button::builder()
                        .label("Generate Code")
                        .css_classes(["suggested-action"])
                        .valign(gtk4::Align::Center)
                        .build();
                    {
                        let s = sender.clone();
                        let sess = session.name.clone();
                        gen_btn.connect_clicked(move |_| {
                            s.input(AppMsg::GenerateObserverToken(sess.clone()));
                        });
                    }
                    obs_row.add_suffix(&gen_btn);
                }
                expander.add_row(&obs_row);

                // Session Controls sub-row
                let controls_row = adw::ActionRow::builder()
                    .title("Session Controls")
                    .build();

                let ctrl_box = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Horizontal)
                    .spacing(8)
                    .valign(gtk4::Align::Center)
                    .build();

                if session.name == model.config.session_name && model.is_running {
                    let sleep_btn = gtk4::Button::builder()
                        .label("Put to Sleep")
                        .css_classes(["warning"])
                        .valign(gtk4::Align::Center)
                        .build();
                    {
                        let s = sender.clone();
                        sleep_btn.connect_clicked(move |_| {
                            s.input(AppMsg::StopService);
                        });
                    }
                    ctrl_box.append(&sleep_btn);
                } else if session.is_exited {
                    let wake_btn = gtk4::Button::builder()
                        .label("Wake / Resurrect")
                        .css_classes(["suggested-action"])
                        .valign(gtk4::Align::Center)
                        .build();
                    {
                        let s = sender.clone();
                        let sess = session.name.clone();
                        wake_btn.connect_clicked(move |_| {
                            s.input(AppMsg::ResurrectSession(sess.clone()));
                        });
                    }
                    ctrl_box.append(&wake_btn);
                }

                let del_btn = gtk4::Button::builder()
                    .label("Delete")
                    .css_classes(["destructive-action"])
                    .valign(gtk4::Align::Center)
                    .build();
                {
                    let s = sender.clone();
                    let sess = session.name.clone();
                    del_btn.connect_clicked(move |_| {
                        s.input(AppMsg::DeleteSession(sess.clone()));
                    });
                }
                ctrl_box.append(&del_btn);

                controls_row.add_suffix(&ctrl_box);
                expander.add_row(&controls_row);

                widgets.sessions_box.append(&expander);
            }
        }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::StartCreation => {
                self.is_creating = true;
                self.progress_fraction = 0.1;
                self.status_log = vec!["[1/5] Initializing terminal environment...".to_string()];

                let config = self.config.clone();
                let password = if self.password_input.trim().is_empty() {
                    "ByAnyMeans".to_string()
                } else {
                    self.password_input.clone()
                };
                let tailscale = self.tailscale_info.clone();
                let s = sender.clone();

                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(200));
                    s.input(AppMsg::ProgressUpdate(
                        0.25,
                        format!("[2/5] Preparing persistent session '{}'...", config.session_name),
                    ));
                    let _ = ZellijManager::ensure_session_running(&config.session_name);

                    thread::sleep(Duration::from_millis(200));
                    if config.require_password || config.serve_to_web {
                        s.input(AppMsg::ProgressUpdate(
                            0.50,
                            "[3/5] Applying SHA-256 security credentials...".to_string(),
                        ));
                        let _ = ZellijManager::set_password(&password);
                    }

                    thread::sleep(Duration::from_millis(200));
                    s.input(AppMsg::ProgressUpdate(
                        0.70,
                        "[4/5] Starting persistent background service...".to_string(),
                    ));
                    let _ = SystemdManager::write_service_unit(&config.session_name, config.port);
                    let _ = SystemdManager::start();

                    if config.auto_start_on_boot {
                        let _ = SystemdManager::enable();
                        let _ = SystemdManager::ensure_linger();
                    }

                    thread::sleep(Duration::from_millis(200));
                    if config.serve_to_web && tailscale.is_logged_in {
                        s.input(AppMsg::ProgressUpdate(
                            0.85,
                            "[5/5] Activating Tailscale Funnel remote tunnel...".to_string(),
                        ));
                        let _ = TailscaleManager::set_funnel(true, config.port);
                    }

                    // Poll until port is alive or timeout
                    let mut online = false;
                    for _ in 0..20 {
                        thread::sleep(Duration::from_millis(150));
                        if ZellijManager::is_web_server_online(config.port) {
                            online = true;
                            break;
                        }
                    }

                    s.input(AppMsg::ProgressUpdate(
                        1.0,
                        if online {
                            "Web server is online and responding!".to_string()
                        } else {
                            "Service started — port may still be initializing.".to_string()
                        },
                    ));
                    thread::sleep(Duration::from_millis(300));

                    let success = online || SystemdManager::is_active();
                    s.input(AppMsg::CreationFinished(
                        success,
                        if success {
                            "\u{2705} Web Terminal successfully created and running!".to_string()
                        } else {
                            "\u{274c} Service failed to start. Check system logs.".to_string()
                        },
                    ));
                });
            }

            AppMsg::ProgressUpdate(fraction, log) => {
                self.progress_fraction = fraction;
                self.status_log.push(log);
            }

            AppMsg::CreationFinished(success, msg) => {
                self.is_creating = false;
                self.is_running = success;
                self.status_log.push(msg);
                self.sessions = ZellijManager::list_all_sessions();
                self.sessions_dirty.set(true);
                self.update_url();
            }

            AppMsg::StopService => {
                let _ = SystemdManager::stop();
                if self.config.serve_to_web {
                    let _ = TailscaleManager::set_funnel(false, self.config.port);
                }
                thread::sleep(Duration::from_millis(500));
                self.is_running = SystemdManager::is_active();
                self.sessions = ZellijManager::list_all_sessions();
                self.sessions_dirty.set(true);
            }

            AppMsg::ToggleServeToWeb(serve_web) => {
                if self.config.serve_to_web == serve_web {
                    return;
                }
                self.config.serve_to_web = serve_web;
                if serve_web {
                    self.config.require_password = true;
                }
                let _ = self.config.save();
                self.update_url();
            }

            AppMsg::TogglePassword(req) => {
                if self.config.require_password == req {
                    return;
                }
                self.config.require_password = req;
                let _ = self.config.save();
            }

            AppMsg::PasswordChanged(new_pw) => {
                if self.password_input == new_pw {
                    return;
                }
                self.password_input = new_pw;
            }

            AppMsg::ToggleAutoStart(auto_start) => {
                if self.config.auto_start_on_boot == auto_start {
                    return;
                }
                self.config.auto_start_on_boot = auto_start;
                let _ = self.config.save();
            }

            AppMsg::PortChanged(new_port) => {
                if self.config.port == new_port {
                    return;
                }
                self.config.port = new_port;
                let _ = self.config.save();
                self.update_url();
            }

            AppMsg::CopyUrl => {
                if let Some(display) = gdk::Display::default() {
                    let clipboard = display.clipboard();
                    clipboard.set_text(&self.current_url);
                }
            }

            AppMsg::OpenBrowser => {
                let url = if self.config.serve_to_web && self.tailscale_info.fqdn.is_some() {
                    self.current_url.clone()
                } else {
                    NetworkManager::get_local_browser_url(self.config.port)
                };
                let _ = Command::new("xdg-open").arg(&url).spawn();
            }

            AppMsg::ShowQrModal => {
                show_qr_dialog(&self.current_url, "Scan with Phone Camera");
            }

            AppMsg::ConnectTailscale => {
                let _ = TailscaleManager::launch_login_in_browser();
            }

            AppMsg::RefreshStatus => {
                self.is_running = SystemdManager::is_active();
                self.tailscale_info = TailscaleManager::get_info();
                self.sessions = ZellijManager::list_all_sessions();
                self.sessions_dirty.set(true);
                self.update_url();
            }

            AppMsg::RequestFreshReset => {
                let _ = SystemdManager::remove_service_unit();
                let _ = ZellijManager::reset_credentials();
                let _ = AppConfig::reset();
                let _ = TailscaleManager::set_funnel(false, self.config.port);

                self.config = AppConfig::default();
                self.is_running = false;
                self.is_creating = false;
                self.password_input = "ByAnyMeans".to_string();
                self.progress_fraction = 0.0;
                self.status_log = vec!["Environment reset to fresh state.".to_string()];
                self.observer_tokens.clear();
                self.observer_status_text = "Generate a read-only passcode to let someone watch your terminal".to_string();
                self.sessions = ZellijManager::list_all_sessions();
                self.sessions_dirty.set(true);
                self.update_url();
            }

            AppMsg::SessionsLoaded(sessions) => {
                self.sessions = sessions;
                self.sessions_dirty.set(true);
            }

            AppMsg::GenerateObserverToken(session_name) => {
                match ZellijManager::create_observer_token(&session_name) {
                    Ok(raw_token) => {
                        sender.input(AppMsg::ObserverTokenGenerated(session_name, raw_token));
                    }
                    Err(err) => {
                        eprintln!("Failed to generate observer token: {:?}", err);
                    }
                }
            }

            AppMsg::ObserverTokenGenerated(session_name, token) => {
                if session_name == self.config.session_name {
                    self.observer_status_text = format!("View-Only Code: {} (Read-Only)", token);
                }
                self.observer_tokens.insert(session_name, token);
                self.sessions_dirty.set(true);
            }

            AppMsg::ShowObserverQr(session_name) => {
                if let Some(token) = self.observer_tokens.get(&session_name) {
                    let desc = format!("Observer Code: {}\nURL: {}", token, self.current_url);
                    show_qr_dialog(&self.current_url, &format!("Observer Access ({})", desc));
                }
            }

            AppMsg::DeleteSession(session_name) => {
                let s = sender.clone();
                thread::spawn(move || {
                    let _ = ZellijManager::delete_session(&session_name);
                    let sessions = ZellijManager::list_all_sessions();
                    s.input(AppMsg::SessionsLoaded(sessions));
                });
            }

            AppMsg::ResurrectSession(session_name) => {
                let s = sender.clone();
                thread::spawn(move || {
                    let _ = ZellijManager::resurrect_session(&session_name);
                    let sessions = ZellijManager::list_all_sessions();
                    s.input(AppMsg::SessionsLoaded(sessions));
                });
            }

            AppMsg::NewSessionNameChanged(name) => {
                if self.new_session_name == name {
                    return;
                }
                self.new_session_name = name;
            }

            AppMsg::CreateNewSession => {
                let name = if self.new_session_name.trim().is_empty() {
                    format!("scry-{}", self.sessions.len() + 1)
                } else {
                    self.new_session_name.trim().to_string()
                };
                let s = sender.clone();
                thread::spawn(move || {
                    let _ = ZellijManager::ensure_session_running(&name);
                    let sessions = ZellijManager::list_all_sessions();
                    s.input(AppMsg::SessionsLoaded(sessions));
                });
                self.new_session_name = format!("scry-{}", self.sessions.len() + 2);
            }
        }
    }
}

impl AppModel {
    fn update_url(&mut self) {
        if self.config.serve_to_web && self.tailscale_info.fqdn.is_some() {
            self.current_url =
                NetworkManager::get_web_url(self.tailscale_info.fqdn.as_ref().unwrap());
        } else {
            self.current_url = NetworkManager::get_lan_url(self.config.port);
        }
    }
}

fn show_qr_dialog(url: &str, title: &str) {
    if let Ok(png_bytes) = QrGenerator::generate_png_bytes(url) {
        let gbytes = gtk4::glib::Bytes::from(&png_bytes);
        let texture = gdk::Texture::from_bytes(&gbytes).ok();

        let window = adw::Window::builder()
            .title(title)
            .default_width(360)
            .default_height(420)
            .modal(true)
            .build();

        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(16)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .halign(gtk4::Align::Center)
            .build();

        let picture = gtk4::Picture::builder()
            .width_request(260)
            .height_request(260)
            .can_shrink(false)
            .build();

        if let Some(tex) = texture {
            picture.set_paintable(Some(&tex));
        }

        let label = gtk4::Label::builder()
            .label(url)
            .wrap(true)
            .css_classes(["caption", "dim-label"])
            .build();

        container.append(&picture);
        container.append(&label);
        window.set_content(Some(&container));
        window.present();
    }
}

fn main() {
    let app = RelmApp::new("org.lilith.Scry");
    app.run::<AppModel>(());
}
