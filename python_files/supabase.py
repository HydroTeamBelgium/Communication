import os
import requests

SUPABASE_URL = "https://ctrbjkapatxjuogfewae.supabase.co"
SUPABASE_KEY = "sb_publishable_aNJfm7INlJE9qMvkMWLnIw_hOWNYvY7"

TABLE_NAME = "SensorData"

temperature = 30.0
humidity = 80.0

url = f"{SUPABASE_URL}/rest/v1/{TABLE_NAME}"

headers = {
"apikey": SUPABASE_KEY,
"Authorization": f"Bearer {SUPABASE_KEY}",
"Content-Type": "application/json",
"Prefer": "return=minimal",
}

payload = {
"temperature": temperature,
"humidity": humidity,
}

try:
    response = requests.post(
        url,
        headers=headers,
        json=payload,
        timeout=15,
        )

    response.raise_for_status()

    print("Data successfully sent to Supabase!")

except requests.RequestException as error:
    print(f"Could not send data: {error}")

    if error.response is not None:
        print("Server response:", error.response.text)