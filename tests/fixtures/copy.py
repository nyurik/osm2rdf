import sys, subprocess, os, shutil


def recreate_directory_structure(src, dest):
    os.makedirs(os.path.join(dest, "src"), exist_ok=True)
    print(f"Copying {src} to {dest}\n------------------------------")
    for root, dirs, files in os.walk(src):
        for file in files:
            if not file.endswith(".osm") and not file.endswith(".osm.pbf"):
                continue
            src_file = os.path.join(root, file)
            dst_filename = os.path.relpath(root, src).replace('/', '_') + '_' + file
            is_osm_pbf = dst_filename.endswith(".osm.pbf")

            if not is_osm_pbf:
                dst_file = os.path.join(dest, "src", dst_filename)
                print(f"cp {src_file} {dst_file}")
                shutil.copy2(src_file, dst_file)
                dst_filename = dst_filename[:-4] + "-gen.osm"

            dst_file = os.path.join(dest, dst_filename)
            if is_osm_pbf:
                print(f"cp {src_file} {dst_file}")
                shutil.copy2(src_file, dst_file)
            else:
                cmd = ["osmium", "cat", "-f", "pbf", "-o", dst_file + ".pbf", src_file]
                print(" ".join(cmd))
                res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                print(res.stdout.decode())


def main():
    if len(sys.argv) != 2:
        print("Usage: python copy.py <libosmium_root_dir>")
        sys.exit(1)

    src_dir = os.path.join(sys.argv[1], "test")
    dest_dir = os.path.join(os.path.dirname(__file__), "libosmium")
    recreate_directory_structure(src_dir, dest_dir)


if __name__ == "__main__":
    main()
