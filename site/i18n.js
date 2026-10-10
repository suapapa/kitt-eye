(() => {
  const STORAGE_KEY = "kitt-eye-lang";
  const SUPPORTED = ["en", "ko", "zh", "ja"];

  const messages = {
    en: {
      metaTitle: "kitt-eye - Desk scanner for AI coding agents",
      metaDescription:
        "Homelab monitoring that turns AI CLI lifecycle events into a K.I.T.T. scanner LED bar over MQTT and Home Assistant.",
      ogDescription:
        "Peripheral awareness for AI coding agents on a Knight Rider LED bar.",
      navPrimary: "Primary",
      navDemo: "Demo",
      navArchitecture: "Architecture",
      navThingiverse: "Thingiverse",
      navGithub: "GitHub",
      langLabel: "Language",
      heroAria: "Introduction",
      heroLead:
        "A homelab monitor that gathers AI coding agent lifecycle events over MQTT and shows the active state on a desk LED bar.",
      heroMotif:
        "Deskterior modeled on the K.I.T.T. scanner from Knight Rider.",
      ctaGithub: "GitHub Repo",
      ctaThingiverse: "Thingiverse",
      demoTitle: "In motion",
      demoBody:
        "Hooks watch Claude, Codex, Cursor, Antigravity, and Hermes. A Go daemon aggregates state over MQTT onto an ESP32 NeoPixel bar.",
      videoTitle: "kitt-eye demo video",
      archTitle: "How the pieces connect",
      archBody:
        "From CLI lifecycle hooks through Unix socket and MQTT to the WS2812B scanner. Open the interactive diagram for the full map.",
      archLinkAria: "Open interactive kitt-eye architecture diagram",
      archImgAlt:
        "kitt-eye architecture diagram: AI CLI hooks to Go publisher to MQTT to ESP32 LED bar",
      archMetaStrong: "Interactive architecture",
      archMetaSpan: "Open diagram",
      pipelineTitle: "Built for the desk, not the dashboard",
      pipelineBody:
        "Fire-and-forget hooks, priority aggregation, and retained MQTT so the bar recovers state on boot.",
      stepHooksTitle: "Lifecycle hooks",
      stepHooksBody:
        "Read-only observers for five AI CLIs. NDJSON over /tmp/kitt-eye.sock, silent on stdout.",
      stepPubTitle: "Go publisher",
      stepPubBody:
        "Multi-session store with TTL janitor. Collapses many agents into one active_state.",
      stepMqttTitle: "MQTT + Home Assistant",
      stepMqttBody:
        "QoS 1 retained topics, LWT availability, and MQTT discovery sensors.",
      stepMcuTitle: "ESP32 scanner",
      stepMcuBody:
        "Rust firmware on C3 or S2 drives an 8-LED WS2812B bar in classic red or colorful themes.",
      linksTitle: "Get the bits",
      linksBody: "Source, printable mount, and the live architecture viewer.",
      tileGithub: "GitHub Repo",
      tileThingiverse: "Thingiverse",
      tileArch: "Architecture",
      tileArchSpan: "Interactive diagram",
      tileDemo: "Demo video",
      footerCopy: "kitt-eye - open source homelab agent monitor",
    },
    ko: {
      metaTitle: "kitt-eye - AI 코딩 에이전트용 데스크테리어",
      metaDescription:
        "AI CLI 생명주기 이벤트를 MQTT와 Home Assistant로 모아 K.I.T.T. 스캐너 LED 바에 띄우는 홈랩 모니터.",
      ogDescription:
        "Knight Rider K.I.T.T. 스캐너를 딴 데스크테리어. AI 코딩 에이전트 상태를 LED로 봅니다.",
      navPrimary: "주 메뉴",
      navDemo: "데모",
      navArchitecture: "아키텍처",
      navThingiverse: "Thingiverse",
      navGithub: "GitHub",
      langLabel: "언어",
      heroAria: "소개",
      heroLead:
        "AI 코딩 에이전트 생명주기 이벤트를 MQTT로 모아서, 책상 LED 바로 현재 상태를 보여 줍니다.",
      heroMotif:
        "Knight Rider의 K.I.T.T. 스캐너를 딴 데스크테리어입니다.",
      ctaGithub: "GitHub 저장소",
      ctaThingiverse: "Thingiverse",
      demoTitle: "동작 모습",
      demoBody:
        "훅이 Claude, Codex, Cursor, Antigravity, Hermes를 봅니다. Go 데몬이 MQTT로 상태를 모아 ESP32 NeoPixel 바에 올립니다.",
      videoTitle: "kitt-eye 데모 영상",
      archTitle: "연결 구조",
      archBody:
        "CLI 생명주기 훅에서 Unix 소켓, MQTT를 거쳐 WS2812B 스캐너로 이어집니다. 자세한 그림은 인터랙티브 다이어그램을 보세요.",
      archLinkAria: "인터랙티브 kitt-eye 아키텍처 다이어그램 열기",
      archImgAlt:
        "kitt-eye 아키텍처 다이어그램: AI CLI 훅, Go 퍼블리셔, MQTT, ESP32 LED 바",
      archMetaStrong: "인터랙티브 아키텍처",
      archMetaSpan: "다이어그램 열기",
      pipelineTitle: "대시보드 말고 책상 위",
      pipelineBody:
        "보내고 끝나는 훅, 우선순위 집계, retained MQTT로 부팅 직후에도 상태를 바로 복구합니다.",
      stepHooksTitle: "생명주기 훅",
      stepHooksBody:
        "다섯 AI CLI용 읽기 전용 옵저버. /tmp/kitt-eye.sock로 NDJSON을 보내고 stdout은 건드리지 않습니다.",
      stepPubTitle: "Go 퍼블리셔",
      stepPubBody:
        "TTL로 정리하는 다중 세션 저장소. 여러 에이전트를 하나의 active_state로 모읍니다.",
      stepMqttTitle: "MQTT + Home Assistant",
      stepMqttBody:
        "QoS 1 retained 토픽, LWT 가용성, MQTT discovery 센서.",
      stepMcuTitle: "ESP32 스캐너",
      stepMcuBody:
        "C3/S2용 Rust 펌웨어가 8 LED WS2812B 바를 classic 빨강 또는 colorful 테마로 돌립니다.",
      linksTitle: "링크",
      linksBody: "소스, 프린트용 마운트, 라이브 아키텍처 뷰어.",
      tileGithub: "GitHub 저장소",
      tileThingiverse: "Thingiverse",
      tileArch: "아키텍처",
      tileArchSpan: "인터랙티브 다이어그램",
      tileDemo: "데모 영상",
      footerCopy: "kitt-eye - 오픈소스 홈랩 에이전트 모니터",
    },
    zh: {
      metaTitle: "kitt-eye - 面向 AI 编程助手的桌面扫描灯",
      metaDescription:
        "将 AI CLI 生命周期事件经 MQTT 与 Home Assistant 显示在 K.I.T.T. 扫描 LED 灯条上的家庭实验室监控。",
      ogDescription: "用 Knight Rider 风格灯条感知 AI 编程助手状态。",
      navPrimary: "主导航",
      navDemo: "演示",
      navArchitecture: "架构",
      navThingiverse: "Thingiverse",
      navGithub: "GitHub",
      langLabel: "语言",
      heroAria: "介绍",
      heroLead:
        "采集 AI 编程助手生命周期事件，经 MQTT 聚合后显示在桌面 LED 灯条上的家庭实验室监控。",
      heroMotif:
        "以《霹雳游侠》K.I.T.T. 扫描灯为灵感的桌面装饰。",
      ctaGithub: "GitHub 仓库",
      ctaThingiverse: "Thingiverse",
      demoTitle: "动态演示",
      demoBody:
        "钩子监听 Claude、Codex、Cursor、Antigravity 与 Hermes。Go 守护进程经 MQTT 聚合状态，显示在 ESP32 NeoPixel 灯条上。",
      videoTitle: "kitt-eye 演示视频",
      archTitle: "各部分如何衔接",
      archBody:
        "从 CLI 生命周期钩子，经 Unix 套接字与 MQTT，再到 WS2812B 扫描灯。打开交互式图查看完整链路。",
      archLinkAria: "打开交互式 kitt-eye 架构图",
      archImgAlt: "kitt-eye 架构图：AI CLI 钩子、Go 发布器、MQTT、ESP32 LED 灯条",
      archMetaStrong: "交互式架构",
      archMetaSpan: "打开图示",
      pipelineTitle: "为桌面而生，不是为仪表盘",
      pipelineBody:
        "即发即忘钩子、优先级聚合与 retained MQTT，上电即可恢复状态。",
      stepHooksTitle: "生命周期钩子",
      stepHooksBody:
        "五个 AI CLI 的只读观察者。经 /tmp/kitt-eye.sock 发送 NDJSON，不污染 stdout。",
      stepPubTitle: "Go 发布器",
      stepPubBody: "带 TTL 清理的多会话存储，将多个助手收敛为单一 active_state。",
      stepMqttTitle: "MQTT + Home Assistant",
      stepMqttBody: "QoS 1 retained 主题、LWT 可用性，以及 MQTT discovery 传感器。",
      stepMcuTitle: "ESP32 扫描灯",
      stepMcuBody:
        "C3/S2 上的 Rust 固件驱动 8 颗 WS2812B，支持经典红色或彩色主题。",
      linksTitle: "获取资源",
      linksBody: "源码、可打印支架与实时架构查看器。",
      tileGithub: "GitHub 仓库",
      tileThingiverse: "Thingiverse",
      tileArch: "架构",
      tileArchSpan: "交互式图示",
      tileDemo: "演示视频",
      footerCopy: "kitt-eye - 开源家庭实验室助手监视器",
    },
    ja: {
      metaTitle: "kitt-eye - AIコーディングエージェント用デスクスキャナー",
      metaDescription:
        "AI CLIのライフサイクルイベントをMQTTとHome Assistant経由でK.I.T.T.スキャナーLEDバーに表示するホームラボ監視。",
      ogDescription:
        "Knight Rider風LEDバーでAIコーディングエージェントの状態を把握。",
      navPrimary: "メインナビ",
      navDemo: "デモ",
      navArchitecture: "アーキテクチャ",
      navThingiverse: "Thingiverse",
      navGithub: "GitHub",
      langLabel: "言語",
      heroAria: "紹介",
      heroLead:
        "AIコーディングエージェントのライフサイクルをMQTTで集約し、机上のLEDバーに表示するホームラブモニターです。",
      heroMotif:
        "Knight RiderのK.I.T.T.スキャナーを模したデスクインテリアです。",
      ctaGithub: "GitHubリポジトリ",
      ctaThingiverse: "Thingiverse",
      demoTitle: "動作の様子",
      demoBody:
        "フックがClaude、Codex、Cursor、Antigravity、Hermesを監視。GoデーモンがMQTTで状態を集約し、ESP32 NeoPixelバーへ届けます。",
      videoTitle: "kitt-eyeデモ動画",
      archTitle: "つながりの全体像",
      archBody:
        "CLIライフサイクルフックからUnixソケット、MQTTを経てWS2812Bスキャナーまで。詳細はインタラクティブ図で。",
      archLinkAria: "インタラクティブなkitt-eyeアーキテクチャ図を開く",
      archImgAlt:
        "kitt-eyeアーキテクチャ図: AI CLIフック、Goパブリッシャー、MQTT、ESP32 LEDバー",
      archMetaStrong: "インタラクティブ構成図",
      archMetaSpan: "図を開く",
      pipelineTitle: "ダッシュボードではなくデスク向け",
      pipelineBody:
        "送信後は待たないフック、優先度集約、retained MQTTで起動直後に状態を復元します。",
      stepHooksTitle: "ライフサイクルフック",
      stepHooksBody:
        "5つのAI CLI向け読み取り専用オブザーバ。/tmp/kitt-eye.sock上のNDJSONで、stdoutは静かです。",
      stepPubTitle: "Goパブリッシャー",
      stepPubBody:
        "TTL掃除付きマルチセッションストア。複数エージェントを一つのactive_stateにまとめます。",
      stepMqttTitle: "MQTT + Home Assistant",
      stepMqttBody:
        "QoS 1 retainedトピック、LWT可用性、MQTT discoveryセンサー。",
      stepMcuTitle: "ESP32スキャナー",
      stepMcuBody:
        "C3/S2向けRustファームウェアが8LEDのWS2812Bバーをクラシック赤またはカラフルテーマで駆動します。",
      linksTitle: "入手する",
      linksBody: "ソース、プリント用マウント、ライブアーキテクチャビューア。",
      tileGithub: "GitHubリポジトリ",
      tileThingiverse: "Thingiverse",
      tileArch: "アーキテクチャ",
      tileArchSpan: "インタラクティブ図",
      tileDemo: "デモ動画",
      footerCopy: "kitt-eye - オープンソースのホームラボエージェントモニター",
    },
  };

  function normalizeLang(raw) {
    if (!raw) return null;
    const lower = String(raw).toLowerCase().replace("_", "-");
    if (SUPPORTED.includes(lower)) return lower;
    const base = lower.split("-")[0];
    if (base === "zh") return "zh";
    if (SUPPORTED.includes(base)) return base;
    return null;
  }

  function detectLang() {
    try {
      const stored = normalizeLang(localStorage.getItem(STORAGE_KEY));
      if (stored) return stored;
    } catch (_) {}

    const candidates = [];
    if (Array.isArray(navigator.languages)) {
      candidates.push(...navigator.languages);
    }
    if (navigator.language) candidates.push(navigator.language);

    for (const item of candidates) {
      const hit = normalizeLang(item);
      if (hit) return hit;
    }
    return "en";
  }

  function applyLang(lang) {
    const dict = messages[lang] || messages.en;
    document.documentElement.lang = lang;

    document.querySelectorAll("[data-i18n]").forEach((el) => {
      const key = el.getAttribute("data-i18n");
      if (!key || dict[key] == null) return;
      el.textContent = dict[key];
    });

    document.querySelectorAll("[data-i18n-aria]").forEach((el) => {
      const key = el.getAttribute("data-i18n-aria");
      if (!key || dict[key] == null) return;
      el.setAttribute("aria-label", dict[key]);
    });

    document.querySelectorAll("[data-i18n-alt]").forEach((el) => {
      const key = el.getAttribute("data-i18n-alt");
      if (!key || dict[key] == null) return;
      el.setAttribute("alt", dict[key]);
    });

    document.querySelectorAll("[data-i18n-title]").forEach((el) => {
      const key = el.getAttribute("data-i18n-title");
      if (!key || dict[key] == null) return;
      el.setAttribute("title", dict[key]);
    });

    if (dict.metaTitle) document.title = dict.metaTitle;

    const metaDesc = document.querySelector('meta[name="description"]');
    if (metaDesc && dict.metaDescription) {
      metaDesc.setAttribute("content", dict.metaDescription);
    }
    const ogDesc = document.querySelector('meta[property="og:description"]');
    if (ogDesc && dict.ogDescription) {
      ogDesc.setAttribute("content", dict.ogDescription);
    }

    const select = document.getElementById("lang-select");
    if (select) {
      if (select.value !== lang) select.value = lang;
      if (dict.langLabel) select.setAttribute("aria-label", dict.langLabel);
    }

    document.querySelectorAll(".lang-switch__btn").forEach((btn) => {
      const active = btn.getAttribute("data-lang") === lang;
      btn.setAttribute("aria-pressed", active ? "true" : "false");
      btn.classList.toggle("is-active", active);
    });

    document.querySelectorAll(".lang-switch__pills").forEach((group) => {
      if (dict.langLabel) group.setAttribute("aria-label", dict.langLabel);
    });
  }

  function setLang(lang) {
    const next = normalizeLang(lang) || "en";
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch (_) {}
    applyLang(next);
  }

  function init() {
    const initial = detectLang();
    applyLang(initial);

    const select = document.getElementById("lang-select");
    if (select) {
      select.addEventListener("change", (e) => setLang(e.target.value));
    }

    document.querySelectorAll(".lang-switch__btn").forEach((btn) => {
      btn.addEventListener("click", () => setLang(btn.getAttribute("data-lang")));
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
