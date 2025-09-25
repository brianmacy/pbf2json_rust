use anyhow::Result;
use lmdb::{Database, Environment, Transaction, WriteFlags};
use std::fs;
use std::path::{Path, PathBuf};

/// Disk-based coordinate storage using LMDB for memory-efficient geometry computation
pub struct CoordinateStorage {
    env: Environment,
    db: Database,
    temp_path: Option<PathBuf>, // Track if we created a temp directory for cleanup
    keep_temp_db: bool,         // Whether to keep the temp database on drop
}

impl CoordinateStorage {
    /// Create coordinate storage at specified path, or temp dir if None
    #[allow(dead_code)]
    pub fn new(db_path: Option<&Path>) -> Result<Self> {
        Self::new_with_cleanup(db_path, false)
    }

    /// Create coordinate storage with specified cleanup behavior
    pub fn new_with_cleanup(db_path: Option<&Path>, keep_temp_db: bool) -> Result<Self> {
        let (path, temp_path) = match db_path {
            Some(path) => (path.to_path_buf(), None),
            None => {
                let temp_dir = tempfile::tempdir()?;
                let path = temp_dir.path().join("coordinates");
                (path, Some(temp_dir.path().to_path_buf()))
            }
        };

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Configure LMDB environment for high performance
        let env = Environment::new()
            .set_flags(lmdb::EnvironmentFlags::NO_SUB_DIR) // Use single file, not directory
            .set_max_readers(126) // Support multiple readers
            .set_map_size(500 * 1024 * 1024 * 1024) // 500GB max map size for planet files
            .open(&path)?;

        let db = env.open_db(None)?;

        Ok(CoordinateStorage {
            env,
            db,
            temp_path,
            keep_temp_db,
        })
    }

    /// Create coordinate storage in default temp location
    #[allow(dead_code)]
    pub fn new_temp() -> Result<Self> {
        Self::new(None)
    }

    /// Store coordinates for a node ID
    #[allow(dead_code)]
    pub fn store_node(&self, node_id: i64, lat: f64, lon: f64) -> Result<()> {
        let mut txn = self.env.begin_rw_txn()?;
        let key = node_id.to_be_bytes();
        let value = [lat.to_be_bytes(), lon.to_be_bytes()].concat();
        txn.put(self.db, &key, &value, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    /// Store multiple coordinates efficiently in a single transaction
    pub fn store_nodes(&self, nodes: &[(i64, f64, f64)]) -> Result<()> {
        let mut txn = self.env.begin_rw_txn()?;
        for &(node_id, lat, lon) in nodes {
            let key = node_id.to_be_bytes();
            let value = [lat.to_be_bytes(), lon.to_be_bytes()].concat();
            txn.put(self.db, &key, &value, WriteFlags::empty())?;
        }
        txn.commit()?;
        Ok(())
    }

    /// Retrieve coordinates for a node ID
    #[allow(dead_code)]
    pub fn get_node(&self, node_id: i64) -> Result<Option<(f64, f64)>> {
        let txn = self.env.begin_ro_txn()?;
        let key = node_id.to_be_bytes();

        match txn.get(self.db, &key) {
            Ok(value) if value.len() == 16 => {
                let lat_bytes: [u8; 8] = value[0..8].try_into().unwrap();
                let lon_bytes: [u8; 8] = value[8..16].try_into().unwrap();
                let lat = f64::from_be_bytes(lat_bytes);
                let lon = f64::from_be_bytes(lon_bytes);
                Ok(Some((lat, lon)))
            }
            Ok(_) => Ok(None), // Invalid data
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Retrieve coordinates for multiple node IDs efficiently in a single transaction
    pub fn get_nodes(&self, node_ids: &[i64]) -> Result<Vec<Option<(f64, f64)>>> {
        let txn = self.env.begin_ro_txn()?;
        let mut result = Vec::with_capacity(node_ids.len());

        for &node_id in node_ids {
            let key = node_id.to_be_bytes();
            match txn.get(self.db, &key) {
                Ok(value) if value.len() == 16 => {
                    let lat_bytes: [u8; 8] = value[0..8].try_into().unwrap();
                    let lon_bytes: [u8; 8] = value[8..16].try_into().unwrap();
                    let lat = f64::from_be_bytes(lat_bytes);
                    let lon = f64::from_be_bytes(lon_bytes);
                    result.push(Some((lat, lon)));
                }
                Ok(_) => result.push(None), // Invalid data
                Err(lmdb::Error::NotFound) => result.push(None),
                Err(e) => return Err(e.into()),
            }
        }

        Ok(result)
    }

    /// Sync all pending writes to disk
    pub fn sync(&self) -> Result<()> {
        self.env.sync(true)?;
        Ok(())
    }
}

impl Drop for CoordinateStorage {
    fn drop(&mut self) {
        // Ensure all data is synced before cleanup
        let _ = self.sync();

        // Clean up temporary directory only if we created one AND keep_temp_db is false
        if let Some(temp_path) = &self.temp_path {
            if !self.keep_temp_db {
                if let Err(e) = fs::remove_dir_all(temp_path) {
                    eprintln!(
                        "Warning: Failed to remove temp database {}: {}",
                        temp_path.display(),
                        e
                    );
                } else {
                    eprintln!(
                        "✅ Temporary coordinate database deleted: {}",
                        temp_path.display()
                    );
                }
            } else {
                eprintln!(
                    "📁 Temporary coordinate database preserved: {}",
                    temp_path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_storage() -> Result<()> {
        let storage = CoordinateStorage::new_temp()?;

        // Store some coordinates
        storage.store_node(123, 40.7128, -74.0060)?; // NYC
        storage.store_node(456, 51.5074, -0.1278)?; // London

        // Retrieve single coordinate
        let nyc = storage.get_node(123)?;
        assert_eq!(nyc, Some((40.7128, -74.0060)));

        // Retrieve non-existent coordinate
        let missing = storage.get_node(999)?;
        assert_eq!(missing, None);

        // Retrieve multiple coordinates
        let coords = storage.get_nodes(&[123, 456, 999])?;
        assert_eq!(coords.len(), 3);
        assert_eq!(coords[0], Some((40.7128, -74.0060)));
        assert_eq!(coords[1], Some((51.5074, -0.1278)));
        assert_eq!(coords[2], None);

        Ok(())
    }

    #[test]
    fn test_batch_operations() -> Result<()> {
        let storage = CoordinateStorage::new_temp()?;

        // Store multiple coordinates in batch
        let nodes = vec![
            (100, 37.7749, -122.4194), // San Francisco
            (200, 34.0522, -118.2437), // Los Angeles
            (300, 40.7589, -73.9851),  // New York (Times Square)
        ];
        storage.store_nodes(&nodes)?;

        // Retrieve them
        let coords = storage.get_nodes(&[100, 200, 300, 400])?;
        assert_eq!(coords.len(), 4);
        assert_eq!(coords[0], Some((37.7749, -122.4194)));
        assert_eq!(coords[1], Some((34.0522, -118.2437)));
        assert_eq!(coords[2], Some((40.7589, -73.9851)));
        assert_eq!(coords[3], None); // Non-existent

        Ok(())
    }
}
