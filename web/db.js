// Local persistence for the browser arcade.
//
// Saves are keyed by a SHA-256 of the ROM, so re-picking the same file finds
// its battery data without any server involvement. ROMs are only kept when the
// user explicitly asks the page to remember them. ROM metadata and bytes live
// in separate stores so listing the recent games never copies 32 MB payloads.

const DB_NAME = "cinnabar-arcade";
const DB_VERSION = 2;

export class SaveStore {
  constructor() {
    this.db = null;
  }

  async open() {
    if (this.db) {
      return this;
    }
    if (!("indexedDB" in window)) {
      throw new Error("IndexedDB is not available in this browser");
    }

    this.db = await new Promise((resolve, reject) => {
      const request = indexedDB.open(DB_NAME, DB_VERSION);
      request.onupgradeneeded = (event) => {
        const db = request.result;
        if (!db.objectStoreNames.contains("saves")) {
          db.createObjectStore("saves", { keyPath: "hash" });
        }
        if (!db.objectStoreNames.contains("roms")) {
          db.createObjectStore("roms", { keyPath: "hash" });
        }
        if (!db.objectStoreNames.contains("romData")) {
          db.createObjectStore("romData", { keyPath: "hash" });
        }
        if (event.oldVersion > 0 && event.oldVersion < 2) {
          migrateRomBytes(request.transaction);
        }
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error ?? new Error("could not open IndexedDB"));
      request.onblocked = () => reject(new Error("IndexedDB is blocked by another tab"));
    });

    return this;
  }

  async getSave(hash) {
    return this._get("saves", hash);
  }

  async putSave(hash, bytes) {
    return this._put("saves", { hash, bytes, updatedAt: Date.now() });
  }

  async deleteSave(hash) {
    return this._delete("saves", hash);
  }

  async getRom(hash) {
    const [meta, data] = await Promise.all([
      this._get("roms", hash),
      this._get("romData", hash),
    ]);
    if (!meta || !data?.bytes) {
      return null;
    }
    return { ...meta, bytes: data.bytes };
  }

  async putRom(record) {
    if (!this.db) {
      return false;
    }
    const { bytes, ...meta } = record;
    const tx = this.db.transaction(["roms", "romData"], "readwrite");
    tx.objectStore("roms").put({ ...meta, updatedAt: Date.now() });
    tx.objectStore("romData").put({ hash: record.hash, bytes });
    await transactionDone(tx);
    return true;
  }

  async listRoms() {
    if (!this.db) {
      return [];
    }
    const store = this.db.transaction("roms", "readonly").objectStore("roms");
    const records = await requestResult(store.getAll());
    return records.sort((left, right) => right.updatedAt - left.updatedAt);
  }

  async deleteRom(hash) {
    if (!this.db) {
      return false;
    }
    const tx = this.db.transaction(["roms", "romData"], "readwrite");
    tx.objectStore("roms").delete(hash);
    tx.objectStore("romData").delete(hash);
    await transactionDone(tx);
    return true;
  }

  async clearRoms() {
    if (!this.db) {
      return;
    }
    const tx = this.db.transaction(["roms", "romData"], "readwrite");
    tx.objectStore("roms").clear();
    tx.objectStore("romData").clear();
    await transactionDone(tx);
  }

  async _get(storeName, hash) {
    if (!this.db) {
      return null;
    }
    const store = this.db.transaction(storeName, "readonly").objectStore(storeName);
    return (await requestResult(store.get(hash))) ?? null;
  }

  async _put(storeName, value) {
    if (!this.db) {
      return false;
    }
    const tx = this.db.transaction(storeName, "readwrite");
    tx.objectStore(storeName).put(value);
    await transactionDone(tx);
    return true;
  }

  async _delete(storeName, hash) {
    if (!this.db) {
      return false;
    }
    const tx = this.db.transaction(storeName, "readwrite");
    tx.objectStore(storeName).delete(hash);
    await transactionDone(tx);
    return true;
  }
}

function migrateRomBytes(transaction) {
  if (!transaction) {
    return;
  }
  const store = transaction.objectStore("roms");
  const dataStore = transaction.objectStore("romData");
  const cursorRequest = store.openCursor();
  cursorRequest.onsuccess = () => {
    const cursor = cursorRequest.result;
    if (!cursor) {
      return;
    }
    const value = cursor.value;
    if (value?.bytes) {
      const { bytes, ...meta } = value;
      dataStore.put({ hash: value.hash, bytes });
      cursor.update(meta);
    }
    cursor.continue();
  };
}

export async function hashRom(bytes) {
  const subtle = globalThis.crypto?.subtle;
  if (subtle) {
    const digest = await subtle.digest("SHA-256", bytes);
    return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join(
      "",
    );
  }

  // A cheap fallback for non-secure contexts. This only needs to be stable on
  // this device; it is not a security boundary.
  let hash = 0x811c9dc5;
  for (let index = 0; index < bytes.length; index += 1) {
    hash ^= bytes[index];
    hash = Math.imul(hash, 0x01000193);
  }
  return `fnv-${(hash >>> 0).toString(16).padStart(8, "0")}`;
}

function requestResult(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function transactionDone(transaction) {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () =>
      reject(transaction.error ?? new Error("IndexedDB transaction failed"));
    transaction.onabort = () =>
      reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  });
}
