import sys, os, shutil


def recreate_directory_structure(src, dest):
    os.makedirs(os.path.join(dest, "src"), exist_ok=True)
    print(f"Copying {src} to {dest}\n------------------------------")
    for root, dirs, files in os.walk(src):
        for file in files:
            if file.endswith(".osm") or file.endswith(".osh"):
                src_file = os.path.join(root, file)
                dst_file = os.path.join(dest, "src", os.path.relpath(root, src).replace('/', '_') + '_' + file)
                print(f"cp {src_file} {dst_file}")
                shutil.copy2(src_file, dst_file)


def main():
    if len(sys.argv) != 2:
        print("Usage: python copy.py <libosmium_root_dir>")
        sys.exit(1)

    src_dir = os.path.join(sys.argv[1], "test")
    dst_dir = os.path.join(os.path.dirname(__file__), "libosmium")
    recreate_directory_structure(src_dir, dst_dir)


if __name__ == "__main__":
    main()
