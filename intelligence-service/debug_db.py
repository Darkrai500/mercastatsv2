import os
import psycopg2

def get_db_url():
    env_path = os.path.join(os.path.dirname(__file__), '..', '.env')
    with open(env_path, 'r') as f:
        for line in f:
            if line.startswith('DATABASE_URL='):
                return line.strip().split('=', 1)[1]
    return None

url = get_db_url()
print(f"URL found: {url[:20]}...")

try:
    conn = psycopg2.connect(url)
    print("Connection successful!")
    conn.close()
except Exception as e:
    print(f"Connection failed: {e}")
