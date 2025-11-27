import os
import psycopg2
from dotenv import load_dotenv

# Load .env from parent directory
load_dotenv(os.path.join(os.path.dirname(__file__), '..', '.env'))

DATABASE_URL = os.getenv('DATABASE_URL')

if not DATABASE_URL:
    print("Error: DATABASE_URL not found in .env")
    exit(1)

def apply_sql():
    try:
        conn = psycopg2.connect(DATABASE_URL)
        cur = conn.cursor()
        
        sql_path = os.path.join(os.path.dirname(__file__), '..', 'sql', 'ml_views.sql')
        try:
            with open(sql_path, 'r', encoding='utf-8') as f:
                sql_content = f.read()
        except UnicodeDecodeError:
            with open(sql_path, 'r', encoding='latin-1') as f:
                sql_content = f.read()
            
        print(f"Applying SQL from {sql_path}...")
        cur.execute(sql_content)
        conn.commit()
        print("SQL applied successfully.")
        
        cur.close()
        conn.close()
    except Exception as e:
        print(f"Error applying SQL: {e}")
        exit(1)

if __name__ == "__main__":
    apply_sql()
