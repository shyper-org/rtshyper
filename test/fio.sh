fio -direct=1 -iodepth 1 -thread -rw=read \
    -ioengine=psync -bs=4M -size=256M -numjobs=1 -runtime=180 -group_reporting \
    -name=randrw_70read_4k
