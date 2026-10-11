#!/bin/sh
# 容器入口:先起虚拟显示(和可选的 VNC),再交给 DS_WebAgent。
# DSWA_ENABLE_VNC=false 可以关掉 noVNC(只想用 API、不需要看浏览器时)。
set -e

: "${DISPLAY:=:99}"
export DISPLAY

mkdir -p /data/logs /data/browser-profile

# 容器被 restart/kill 后 Xvfb 的 lock 与 socket 会残留(不在挂载卷),
# 不清掉的话新 Xvfb 会报 Server is already active for display 99 直接退出,
# 之后 Chromium 连不上 DISPLAY 就启动失败。启动前先清残留。
rm -f "/tmp/.X{DISPLAY#:}-lock" "/tmp/.X11-unix/X${DISPLAY#:}"

# 虚拟显示:DS_WebAgent 强制有头模式,容器里没有物理屏幕,所以必须有 Xvfb
Xvfb "$DISPLAY" -screen 0 "${DSWA_SCREEN:-1440x900x24}" -nolisten tcp &

# 等 X socket 就绪(最多 10 秒),避免 Chromium 起来时 DISPLAY 还没准备好
for _ in $(seq 1 20); do
    [ -e "/tmp/.X11-unix/X${DISPLAY#:}" ] && break
    sleep 0.5
done

if [ "${DSWA_ENABLE_VNC:-true}" = "true" ]; then
    x11vnc -display "$DISPLAY" -nopw -forever -shared -rfbport 5900 -bg -o /tmp/x11vnc.log >/dev/null 2>&1 || true
    if [ -d /usr/share/novnc ]; then
        websockify --web=/usr/share/novnc 6080 localhost:5900 >/tmp/websockify.log 2>&1 &
        echo "noVNC 已启动: http://0.0.0.0:6080/vnc.html"
    fi
fi

echo "DS_WebAgent 启动,配置文件 ${DSWA_CONFIG:-/app/config.docker.toml}"
exec /app/ds_webagent --config "${DSWA_CONFIG:-/app/config.docker.toml}"
