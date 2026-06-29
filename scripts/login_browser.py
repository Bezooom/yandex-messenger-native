#!/usr/bin/env python3
"""
Browser-based Yandex Passport login.
Opens a Chromium window, user logs in, session cookies are saved
to ~/.config/yandex-messenger-native/session.json
"""
import json
import os
import sys
import time

def get_config_dir():
    xdg = os.environ.get("XDG_CONFIG_HOME")
    if xdg:
        return os.path.join(xdg, "yandex-messenger-native")
    return os.path.expanduser("~/.config/yandex-messenger-native")

def main():
    try:
        from playwright.sync_api import sync_playwright
    except ImportError:
        print("ERROR: playwright not installed. Run: pip install playwright && playwright install chromium")
        sys.exit(1)

    config_dir = get_config_dir()
    os.makedirs(config_dir, exist_ok=True)
    session_file = os.path.join(config_dir, "session.json")

    print("=" * 60)
    print("  Yandex Messenger — Browser Login")
    print("=" * 60)
    print()
    print("A browser window will open. Log in to your Yandex account.")
    print("After login, the session will be saved automatically.")
    print()

    with sync_playwright() as pw:
        # Use persistent context to let user see and interact with the browser
        user_data_dir = os.path.join(config_dir, "playwright_profile")
        browser = pw.chromium.launch_persistent_context(
            user_data_dir=user_data_dir,
            headless=False,
            args=["--disable-blink-features=AutomationControlled"],
            viewport={"width": 1024, "height": 768},
        )

        page = browser.pages[0] if browser.pages else browser.new_page()

        # Navigate to Yandex Messenger which will redirect to Passport if not logged in
        page.goto("https://yandex.ru/chat", wait_until="domcontentloaded", timeout=30000)

        print("Waiting for login... (close the browser window when done)")
        print()

        # Poll for Session_id cookie
        session_cookies = None
        csrf_token = None
        max_wait = 300  # 5 minutes
        start = time.time()

        while time.time() - start < max_wait:
            try:
                cookies = browser.cookies()
                session_id = None
                sessionid2 = None
                yandexuid = None

                for c in cookies:
                    if c["name"] == "Session_id" and ".yandex.ru" in c.get("domain", ""):
                        session_id = c["value"]
                    if c["name"] == "sessionid2" and ".yandex.ru" in c.get("domain", ""):
                        sessionid2 = c["value"]
                    if c["name"] == "yandexuid" and ".yandex.ru" in c.get("domain", ""):
                        yandexuid = c["value"]

                if session_id:
                    # Got session! Try to get CSRF token too
                    try:
                        resp = page.evaluate("""
                            async () => {
                                const r = await fetch('https://yandex.ru/messenger/api/registry/csrf-token/', {
                                    credentials: 'include'
                                });
                                return await r.json();
                            }
                        """)
                        csrf_token = resp.get("token", "")
                    except:
                        csrf_token = ""

                    # Save all relevant cookies
                    session_cookies = {}
                    for c in cookies:
                        if ".yandex.ru" in c.get("domain", "") or "yandex.ru" in c.get("domain", ""):
                            session_cookies[c["name"]] = c["value"]

                    print(f"✓ Session acquired! (Session_id: {session_id[:20]}...)")
                    break

                # Check if browser is still open
                if not browser.pages:
                    print("Browser closed by user.")
                    break

            except Exception as e:
                if "Target page, context or browser has been closed" in str(e):
                    break

            time.sleep(2)

        try:
            browser.close()
        except:
            pass

    if session_cookies:
        session_data = {
            "cookies": session_cookies,
            "csrf_token": csrf_token or "",
            "saved_at": int(time.time()),
        }

        with open(session_file, "w") as f:
            json.dump(session_data, f, indent=2)

        print(f"\n✓ Session saved to {session_file}")
        print(f"  Cookies: {len(session_cookies)} items")
        print(f"  CSRF token: {'yes' if csrf_token else 'no'}")
        print("\nYou can now run the messenger app.")
    else:
        print("\n✗ Login failed or timed out.")
        print("  Please try again.")
        sys.exit(1)

if __name__ == "__main__":
    main()
