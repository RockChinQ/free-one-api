/* SEO：Docsify 是单页应用，切页时同步 <title> / description / canonical / OG / Twitter / <html lang>。 */
(function () {
  var BASE = "https://free-one-api.rockchin.top/";
  var OG_IMAGE = BASE + "zh-CN/assets/load_balance.png";

  var ROUTES = {
    "/zh-CN/": ["Free One API 中文文档 · 把 LLM 逆向工程库统一成标准 OpenAI API", "Free One API 中文文档总览：三分钟上手、支持的适配器与核心能力。把各种 LLM 逆向工程库统一封装成标准 OpenAI API，支持流式输出、工具调用与多通道负载均衡。", "zh-CN"],
    "/zh-CN/Setup": ["部署 · Free One API 文档", "Free One API 部署教程：Docker 与 Docker Compose 启动、源码自建镜像、手动部署、systemd 常驻、Nginx 与 Caddy 反向代理（含流式不缓冲配置）、升级与排错。", "zh-CN"],
    "/zh-CN/Usage": ["使用 · Free One API 文档", "Free One API 使用方法：创建渠道与 API Key，用标准 OpenAI 客户端接入，Python 流式示例，以及 tools / tool_choice 工具调用的完整两轮流程。", "zh-CN"],
    "/zh-CN/Adapters": ["适配器 · Free One API 文档", "Free One API 支持的适配器与配置方式：ChatGPT、Claude、Bard、gpt4free、HuggingChat、通义千问，以及本仓库自带的 ds-webagent/deepseek-web 转发适配器。", "zh-CN"],
    "/zh-CN/DSWebAgent": ["接入 DS_WebAgent：DeepSeek 网页版渠道 · Free One API 文档", "用 DS_WebAgent 把 DeepSeek 网页版接成免费渠道：无需 cookie 或 session token，真实浏览器驱动；含 docker-compose 部署、noVNC 首次登录、渠道配置、配置热修改与工具调用示例。", "zh-CN"],
    "/zh-CN/Config": ["配置文件 · Free One API 文档", "Free One API 的 data/config.yaml 全字段说明：adapters、database、router.token、watchdog.heartbeat、random_ad 与 web.frontend_path。", "zh-CN"],
    "/zh-CN/Demo": ["Demo 测试 · Free One API 文档", "Free One API 在线 Demo：可以登录并修改通道与 Key 数据，每 30 分钟重置；另附只读测试通道。", "zh-CN"],
    "/zh-CN/FAQ": ["常见问题 · Free One API 文档", "Free One API 排错速查：管理页空白、数据丢失、端口占用、渠道测试通过但请求失败、No suitable channel found、流式不逐字返回，以及 DS_WebAgent 相关故障。", "zh-CN"],
    "/en/": ["Free One API Documentation · Unified OpenAI API for LLM libraries", "Free One API documentation overview: quickstart, supported adapters and core features. Wrap every reverse-engineered LLM library behind one standard OpenAI API with streaming, tool calling and load balancing.", "en"],
    "/en/Setup": ["Deployment · Free One API Documentation", "Deploy Free One API: Docker and Docker Compose, building the image from source, manual install, a systemd unit, and Nginx / Caddy reverse proxy settings that keep streaming unbuffered.", "en"],
    "/en/Usage": ["Usage · Free One API Documentation", "Using Free One API: create channels and API keys, connect any OpenAI client, Python streaming example, and the full two-round tools / tool_choice calling flow.", "en"],
    "/en/Adapters": ["Adapters · Free One API Documentation", "Adapters shipped with Free One API and how to configure them: ChatGPT, Claude, Bard, gpt4free, HuggingChat, TongYi QianWen, plus the bundled ds-webagent/deepseek-web forwarder.", "en"],
    "/en/DSWebAgent": ["DS_WebAgent (DeepSeek web) · Free One API Documentation", "Turn the DeepSeek web edition into a channel with DS_WebAgent: no cookie or session token, driven through a real browser. Docker compose deployment, noVNC first login, channel config, hot config changes and tool calling.", "en"],
    "/en/Config": ["Config File · Free One API Documentation", "Every field of data/config.yaml: adapters, database, router.token, watchdog.heartbeat, random_ad and web.frontend_path.", "en"],
    "/en/Demo": ["Demo · Free One API Documentation", "The public Free One API demo: log in and change channels and keys, reset every 30 minutes; plus a read-only test channel.", "en"],
    "/en/FAQ": ["FAQ · Free One API Documentation", "Free One API troubleshooting: blank admin page, lost data, busy ports, channel test passes but requests fail, no suitable channel found, streaming not streaming, and DS_WebAgent issues.", "en"],
  };

  function upsert(attr, key, content) {
    if (!content) { return; }
    var el = document.head.querySelector("meta[" + attr + "=\"" + key + "\"]");
    if (!el) {
      el = document.createElement("meta");
      el.setAttribute(attr, key);
      document.head.appendChild(el);
    }
    el.setAttribute("content", content);
  }

  function upsertLink(rel, href, hreflang) {
    var sel = "link[rel=\"" + rel + "\"]";
    if (hreflang) { sel += "[hreflang=\"" + hreflang + "\"]"; }
    var el = document.head.querySelector(sel);
    if (!el) {
      el = document.createElement("link");
      el.setAttribute("rel", rel);
      if (hreflang) { el.setAttribute("hreflang", hreflang); }
      document.head.appendChild(el);
    }
    el.setAttribute("href", href);
  }

  function norm(hash) {
    var h = hash || "";
    if (h.charAt(0) === "#") { h = h.slice(1); }
    var q = h.indexOf("?");
    if (q >= 0) { h = h.slice(0, q); }
    if (!h || h === "/") { h = "/zh-CN/"; }
    if (h.charAt(0) !== "/") { h = "/" + h; }
    if (ROUTES[h]) { return h; }
    var parts = h.split("/").filter(function (x) { return x.length > 0; });
    var key = "/" + parts.join("/");
    if (ROUTES[key]) { return key; }
    if (parts.length > 0 && parts[0] === "en") { return "/en/"; }
    return "/zh-CN/";
  }

  function apply() {
    var route = norm(window.location.hash);
    var meta = ROUTES[route] || ROUTES["/zh-CN/"];
    var title = meta[0], desc = meta[1], lang = meta[2] || "zh-CN";
    var url = BASE + "#" + route;
    var locale = lang === "en" ? "en_US" : "zh_CN";

    document.title = title;
    document.documentElement.setAttribute("lang", lang);
    upsert("name", "description", desc);
    upsertLink("canonical", url);
    upsertLink("alternate", BASE + "#/zh-CN/", "zh-CN");
    upsertLink("alternate", BASE + "#/en/", "en");
    upsertLink("alternate", BASE, "x-default");
    upsert("property", "og:title", title);
    upsert("property", "og:description", desc);
    upsert("property", "og:url", url);
    upsert("property", "og:image", OG_IMAGE);
    upsert("property", "og:locale", locale);
    upsert("name", "twitter:title", title);
    upsert("name", "twitter:description", desc);
    upsert("name", "twitter:url", url);
    upsert("name", "twitter:image", OG_IMAGE);
  }

  apply();
  window.addEventListener("hashchange", function () { window.setTimeout(apply, 80); });
  window.addEventListener("load", apply);
  if (document.body && window.MutationObserver) {
    new window.MutationObserver(function () { window.setTimeout(apply, 60); }).observe(document.body, { childList: true, subtree: true });
  }
  window.setInterval(apply, 1500);
})();
